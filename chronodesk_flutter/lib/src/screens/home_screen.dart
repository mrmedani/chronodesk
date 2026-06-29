import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:typed_data';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:ffi/ffi.dart';
import '../ffi/native.dart' as native;
import '../update_checker.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  String _peerId = '';
  String _signalingAddr = '';
  bool _connected = false;
  bool _isHost = false;
  bool _connecting = false;
  ui.Image? _frameImage;
  int _frameW = 0;
  int _frameH = 0;
  Timer? _pollTimer;
  Timer? _frameTimer;
  final TextEditingController _idController = TextEditingController();
  final TextEditingController _addrController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _signalingAddr = native.getConfig('signaling_addr');
    if (_signalingAddr.isEmpty) _signalingAddr = '144.24.201.196:21116';
    _addrController.text = _signalingAddr;
    native.chronodeskInit();
    Future.delayed(const Duration(milliseconds: 500), () {
      _peerId = native.getPeerId();
      setState(() {});
    });
    Future.delayed(const Duration(seconds: 2), _autoCheckUpdate);
    _pollTimer = Timer.periodic(const Duration(milliseconds: 50), (_) => _pollEvents());
    _frameTimer = Timer.periodic(const Duration(milliseconds: 33), (_) => _pollFrame());
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    _frameTimer?.cancel();
    _idController.dispose();
    _addrController.dispose();
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

  void _showSettings() {
    _addrController.text = native.getConfig('signaling_addr');
    if (_addrController.text.isEmpty) _addrController.text = '144.24.201.196:21116';
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Settings'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const Text('Signaling Server', style: TextStyle(fontSize: 13, color: Colors.grey)),
            const SizedBox(height: 8),
            TextField(
              controller: _addrController,
              style: const TextStyle(fontSize: 16),
              textAlign: TextAlign.center,
              decoration: InputDecoration(
                hintText: 'host:port',
                filled: true,
                fillColor: Colors.grey.shade900,
                border: OutlineInputBorder(borderRadius: BorderRadius.circular(8)),
              ),
            ),
            const SizedBox(height: 12),
            Text(
              'Restart the app after changing this address.',
              style: TextStyle(fontSize: 12, color: Colors.grey.shade500),
            ),
            const SizedBox(height: 24),
            const Divider(color: Colors.grey),
            const SizedBox(height: 8),
            TextButton.icon(
              onPressed: () => _checkUpdate(ctx),
              icon: const Icon(Icons.system_update, size: 18),
              label: const Text('Check for Updates'),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Close'),
          ),
          FilledButton(
            onPressed: () {
              native.setConfig('signaling_addr', _addrController.text.trim());
              setState(() => _signalingAddr = _addrController.text.trim());
              Navigator.of(ctx).pop();
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('Server address saved. Restart to apply.')),
              );
            },
            child: const Text('Save'),
          ),
        ],
      ),
    );
  }

  void _autoCheckUpdate() {
    checkForUpdate().then((update) {
      if (!mounted || update == null) return;
      showDialog(
        context: context,
        builder: (ctx) => AlertDialog(
          title: Row(children: [
            Icon(Icons.system_update, color: Colors.cyan.shade300, size: 22),
            const SizedBox(width: 8),
            Text('Update v${update.version}'),
          ]),
          content: Text('A new version is available. Download and install now?'),
          actions: [
            TextButton(onPressed: () => Navigator.of(ctx).pop(), child: const Text('Later')),
            FilledButton.icon(
              onPressed: () {
                Navigator.of(ctx).pop();
                _performUpdate(update.url);
              },
              icon: const Icon(Icons.download, size: 18),
              label: const Text('Update'),
            ),
          ],
        ),
      );
    });
  }

  void _checkUpdate(BuildContext settingsCtx) {
    showDialog(
      context: settingsCtx,
      barrierDismissible: false,
      builder: (ctx) => AlertDialog(
        title: const Text('Checking for Updates'),
        content: const Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2)),
            SizedBox(width: 16),
            Text('Checking...'),
          ],
        ),
      ),
    );
    checkForUpdate().then((update) {
      Navigator.of(settingsCtx).pop();
      if (update == null) {
        if (mounted) {
          showDialog(
            context: context,
            builder: (ctx) => AlertDialog(
              title: const Text('Up to Date'),
              content:             Text('CHRONODESK v$currentVersion is the latest version.'),
              actions: [FilledButton(onPressed: () => Navigator.of(ctx).pop(), child: const Text('OK'))],
            ),
          );
        }
        return;
      }
      if (!mounted) return;
      showDialog(
        context: context,
        builder: (ctx) => AlertDialog(
          title: Text('Update Available: v${update.version}'),
          content: SingleChildScrollView(
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text('A new version is available.'),
                if (update.notes.isNotEmpty) ...[
                  const SizedBox(height: 12),
                  const Text('Release notes:', style: TextStyle(fontWeight: FontWeight.bold)),
                  const SizedBox(height: 4),
                  Text(update.notes, style: const TextStyle(fontSize: 12)),
                ],
              ],
            ),
          ),
          actions: [
            TextButton(onPressed: () => Navigator.of(ctx).pop(), child: const Text('Later')),
            FilledButton.icon(
              onPressed: () {
                Navigator.of(ctx).pop();
                _performUpdate(update.url);
              },
              icon: const Icon(Icons.download, size: 18),
              label: const Text('Download Update'),
            ),
          ],
        ),
      );
    }).catchError((_) {
      Navigator.of(settingsCtx).pop();
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Failed to check for updates. Check your internet connection.')),
        );
      }
    });
  }

  void _performUpdate(String url) {
    final progressState = ValueNotifier<double>(0.0);
    showDialog(
      context: context,
      barrierDismissible: false,
      builder: (ctx) => AlertDialog(
        title: const Text('Downloading Update...'),
        content: ValueListenableBuilder<double>(
          valueListenable: progressState,
          builder: (ctx, progress, _) => Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Row(
                children: [
                  SizedBox(width: 18, height: 18, child: CircularProgressIndicator(strokeWidth: 2, value: progress > 0 ? progress : null)),
                  const SizedBox(width: 16),
                  Text(progress > 0 ? '${(progress * 100).toInt()}%' : 'Downloading...'),
                ],
              ),
              if (progress > 0) ...[
                const SizedBox(height: 8),
                LinearProgressIndicator(value: progress),
              ],
            ],
          ),
        ),
      ),
    );
    downloadAndApplyUpdate((received, total) {
      progressState.value = received / total;
    }).catchError((e) {
      if (mounted) {
        Navigator.of(context).pop();
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Update failed: $e')),
        );
      }
    });
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
        actions: [
          IconButton(
            icon: Icon(Icons.settings, color: Colors.grey.shade400),
            onPressed: _showSettings,
            tooltip: 'Settings',
          ),
        ],
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
          const SizedBox(height: 8),
          Text(
            'Server: $_signalingAddr',
            style: TextStyle(fontSize: 11, color: Colors.grey.shade600),
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
      ],
    );
  }
}
