import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:typed_data';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:ffi/ffi.dart';
import '../ffi/native.dart' as native;

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  String _peerId = '';
  bool _connected = false;
  bool _isHost = false;
  bool _connecting = false;
  ui.Image? _frameImage;
  int _frameW = 0;
  int _frameH = 0;
  Timer? _pollTimer;
  Timer? _frameTimer;
  final TextEditingController _idController = TextEditingController();

  @override
  void initState() {
    super.initState();
    final addr = '127.0.0.1:21116'.toNativeUtf8();
    native.chronodeskInit(addr);
    calloc.free(addr);
    Future.delayed(const Duration(milliseconds: 500), () {
      _peerId = native.getPeerId();
      setState(() {});
    });
    _pollTimer = Timer.periodic(const Duration(milliseconds: 50), (_) => _pollEvents());
    _frameTimer = Timer.periodic(const Duration(milliseconds: 33), (_) => _pollFrame());
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    _frameTimer?.cancel();
    _idController.dispose();
    super.dispose();
  }

  void _pollEvents() {
    while (true) {
      final ev = native.pollEvent();
      if (ev == null) break;
      _handleEvent(ev);
    }
  }

  void _handleEvent(String json) {
    try {
      final map = jsonDecode(json) as Map<String, dynamic>;
      final type = map['type'] as String?;
      if (type == null) return;
      switch (type) {
        case 'connection_request':
          final from = map['from'] as String? ?? '';
          if (mounted) _showConnectionRequest(from);
        case 'connected':
          setState(() {
            _connected = true;
            _connecting = false;
          });
        case 'disconnected':
          setState(() {
            _connected = false;
            _isHost = false;
            _frameImage = null;
          });
        case 'connecting':
          setState(() => _connecting = true);
        case 'error':
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text('Error: ${map['msg'] ?? ''}')),
            );
          }
      }
    } catch (_) {}
  }

  void _pollFrame() {
    if (!_connected || _isHost) return;
    final data = calloc<Pointer<Uint8>>();
    final len = calloc<Int32>();
    final w = calloc<Int32>();
    final h = calloc<Int32>();
    try {
      if (native.getFrame(data, len, w, h)) {
        final length = len.value;
        final width = w.value;
        final height = h.value;
        if (length > 0 && width > 0 && height > 0) {
          final bytes = Uint8List.fromList(
            data.value.asTypedList(length),
          );
          native.chronodeskFreeFrame(data.value);
          _decodeFrame(bytes, width, height);
        }
      }
    } catch (_) {
      native.chronodeskFreeFrame(data.value);
    } finally {
      calloc.free(data);
      calloc.free(len);
      calloc.free(w);
      calloc.free(h);
    }
  }

  Future<void> _decodeFrame(Uint8List rgba, int w, int h) async {
    final completer = Completer<ui.Image>();
    ui.decodeImageFromPixels(rgba, w, h, ui.PixelFormat.rgba8888, (img) {
      completer.complete(img);
    });
    final img = await completer.future;
    if (!mounted) return;
    setState(() {
      _frameImage = img;
      _frameW = w;
      _frameH = h;
    });
  }

  void _showConnectionRequest(String from) {
    showDialog(
      context: context,
      barrierDismissible: false,
      builder: (ctx) => AlertDialog(
        title: const Text('Incoming Connection'),
        content: Text('User "$from" wants to connect to your computer.'),
        actions: [
          TextButton(
            onPressed: () {
              native.chronodeskDeny();
              Navigator.of(ctx).pop();
            },
            child: const Text('Deny'),
          ),
          FilledButton(
            onPressed: () {
              native.chronodeskAccept();
              setState(() => _isHost = true);
              Navigator.of(ctx).pop();
            },
            child: const Text('Accept'),
          ),
        ],
      ),
    );
  }

  void _connect() {
    final target = _idController.text.trim();
    if (target.isEmpty || target == _peerId) return;
    final ptr = target.toNativeUtf8();
    native.chronodeskConnect(ptr);
    calloc.free(ptr);
  }

  void _disconnect() {
    native.chronodeskDisconnect();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: const Color(0xFF1A1A2E),
      appBar: AppBar(
        title: const Text('CHRONODESK', style: TextStyle(fontWeight: FontWeight.bold, letterSpacing: 2)),
        centerTitle: true,
        backgroundColor: const Color(0xFF16213E),
        foregroundColor: Colors.white,
        elevation: 0,
      ),
      body: _connected && !_isHost && _frameImage != null
          ? _buildRemoteView()
          : _buildHomeView(),
    );
  }

  Widget _buildHomeView() {
    return SingleChildScrollView(
      padding: const EdgeInsets.all(24),
      child: Column(
        children: [
          const SizedBox(height: 24),
          Icon(Icons.desktop_windows, size: 64, color: Colors.blueGrey.shade300),
          const SizedBox(height: 32),
          Text(
            'Your ID',
            style: TextStyle(fontSize: 14, color: Colors.grey.shade400, letterSpacing: 1),
          ),
          const SizedBox(height: 8),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 12),
            decoration: BoxDecoration(
              color: const Color(0xFF16213E),
              borderRadius: BorderRadius.circular(12),
              border: Border.all(color: Colors.blueGrey.shade700),
            ),
            child: SelectableText(
              _peerId.isEmpty ? 'Initializing...' : _peerId,
              style: const TextStyle(fontSize: 32, fontWeight: FontWeight.bold, letterSpacing: 4, color: Colors.white),
            ),
          ),
          if (!_connected) ...[
            const SizedBox(height: 48),
            Text(
              'Remote ID',
              style: TextStyle(fontSize: 14, color: Colors.grey.shade400, letterSpacing: 1),
            ),
            const SizedBox(height: 8),
            SizedBox(
              width: 300,
              child: TextField(
                controller: _idController,
                style: const TextStyle(color: Colors.white, fontSize: 18, letterSpacing: 2),
                textAlign: TextAlign.center,
                decoration: InputDecoration(
                  hintText: 'Enter peer ID',
                  hintStyle: TextStyle(color: Colors.grey.shade600),
                  filled: true,
                  fillColor: const Color(0xFF16213E),
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(12),
                    borderSide: BorderSide(color: Colors.blueGrey.shade700),
                  ),
                  enabledBorder: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(12),
                    borderSide: BorderSide(color: Colors.blueGrey.shade700),
                  ),
                ),
                onSubmitted: (_) => _connect(),
              ),
            ),
            const SizedBox(height: 16),
            SizedBox(
              width: 200,
              child: FilledButton.icon(
                onPressed: _connecting ? null : _connect,
                icon: _connecting
                    ? const SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2))
                    : const Icon(Icons.cast_connected),
                label: Text(_connecting ? 'Connecting...' : 'Connect'),
                style: FilledButton.styleFrom(
                  padding: const EdgeInsets.all(14),
                  backgroundColor: Colors.blueGrey,
                ),
              ),
            ),
          ] else ...[
            const SizedBox(height: 24),
            SizedBox(
              width: 200,
              child: OutlinedButton.icon(
                onPressed: _disconnect,
                icon: const Icon(Icons.link_off),
                label: const Text('Disconnect'),
                style: OutlinedButton.styleFrom(
                  foregroundColor: Colors.red.shade300,
                  side: BorderSide(color: Colors.red.shade300),
                  padding: const EdgeInsets.all(14),
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildRemoteView() {
    return Column(
      children: [
        Container(
          color: const Color(0xFF0F0F1A),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
            child: Row(
              children: [
                Icon(Icons.monitor, color: Colors.green.shade400, size: 18),
                const SizedBox(width: 8),
                Text('Remote Desktop', style: TextStyle(color: Colors.grey.shade300)),
                const Spacer(),
                IconButton(
                  icon: Icon(Icons.close, color: Colors.red.shade300),
                  onPressed: _disconnect,
                  tooltip: 'Disconnect',
                ),
              ],
            ),
          ),
        ),
        Expanded(
          child: GestureDetector(
            onScaleUpdate: (details) {},
            child: InteractiveViewer(
              child: Center(
                child: _frameImage != null
                    ? RawImage(
                        image: _frameImage,
                        fit: BoxFit.contain,
                        width: _frameW.toDouble(),
                        height: _frameH.toDouble(),
                      )
                    : const CircularProgressIndicator(),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
