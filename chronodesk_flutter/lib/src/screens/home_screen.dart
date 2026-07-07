import 'dart:async';
import 'dart:convert';
import 'dart:ffi' hide Size;
import 'dart:io';
import 'dart:ui' as ui;
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:ffi/ffi.dart';
import 'package:file_picker/file_picker.dart';
import 'package:path_provider/path_provider.dart';
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
  bool _updateDialogOpen = false;
  bool _isUpdating = false;
  bool _updateAvailable = false;
  bool _audioMuted = false;
  bool _captureKeyboard = true;
  int _rttMs = 0;
  int _qualityLevel = 85;
  int _targetFps = 30;
  ui.Image? _frameImage;
  int _frameW = 0;
  int _frameH = 0;
  Timer? _pollTimer;
  Timer? _frameTimer;
  Timer? _connectTimer;
  Timer? _updateTimer;
  Map<String, Map<String, dynamic>> _activeTransfers = {};
  final TextEditingController _idController = TextEditingController();
  final TextEditingController _addrController = TextEditingController();
  final TextEditingController _turnUrlController = TextEditingController();
  final TextEditingController _turnUserController = TextEditingController();
  final TextEditingController _turnPassController = TextEditingController();
  final TextEditingController _downloadDirController = TextEditingController();
  final TransformationController _transformController = TransformationController();

  @override
  void initState() {
    super.initState();
    _signalingAddr = native.getConfig('signaling_addr');
    if (_signalingAddr.isEmpty) _signalingAddr = '82.70.239.217:21116';
    _addrController.text = _signalingAddr;
    native.chronodeskInit();
    Future.delayed(const Duration(milliseconds: 500), () {
      _peerId = native.getPeerId();
      if (mounted) setState(() {});
    });
    Future.delayed(const Duration(seconds: 2), _autoCheckUpdate);
    _updateTimer = Timer.periodic(const Duration(minutes: 30), (_) => _autoCheckUpdate());
    _pollTimer = Timer.periodic(const Duration(milliseconds: 50), (_) => _pollEvents());
    _frameTimer = Timer.periodic(const Duration(milliseconds: 33), (_) => _pollFrame());
  }

  @override
  void dispose() {
    _pollTimer?.cancel();
    _frameTimer?.cancel();
    _connectTimer?.cancel();
    _updateTimer?.cancel();
    _idController.dispose();
    _addrController.dispose();
    _turnUrlController.dispose();
    _turnUserController.dispose();
    _turnPassController.dispose();
    _downloadDirController.dispose();
    _transformController.dispose();
    super.dispose();
  }

  void _pollEvents() {
    var guard = 100;
    while (guard > 0) {
      guard--;
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
          _connectTimer?.cancel();
          setState(() {
            _connected = true;
            _connecting = false;
          });
        case 'disconnected':
          _connectTimer?.cancel();
          setState(() {
            _connected = false;
            _isHost = false;
            _connecting = false;
            _frameImage?.dispose();
            _frameImage = null;
            _rttMs = 0;
          });
        case 'connecting':
          setState(() => _connecting = true);
          _connectTimer?.cancel();
          _connectTimer = Timer(const Duration(seconds: 30), () {
            if (!mounted) return;
            native.chronodeskDisconnect();
            setState(() => _connecting = false);
            ScaffoldMessenger.of(context).showSnackBar(
              const SnackBar(content: Text('Connection timeout: peer unreachable')),
            );
          });
        case 'quality':
          setState(() {
            _rttMs = map['rtt'] as int? ?? 0;
            _qualityLevel = map['quality'] as int? ?? 85;
            _targetFps = map['fps'] as int? ?? 30;
          });
        case 'error':
          _connectTimer?.cancel();
          setState(() => _connecting = false);
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text('Error: ${map['msg'] ?? ''}')),
            );
          }
        case 'file_request':
        case 'file_progress':
        case 'file_complete':
        case 'file_sent':
        case 'file_rejected':
        case 'file_cancelled':
        case 'file_error':
          _handleFileTransferEvent(json);
        default:
          break;
      }
    } catch (e) {
      debugPrint('_handleEvent error: $e');
    }
  }

  void _pollFrame() {
    if (!_connected || _isHost) return;
    final data = calloc<Pointer<Uint8>>();
    final len = calloc<Int32>();
    final w = calloc<Int32>();
    final h = calloc<Int32>();
    var frameFreed = false;
    try {
      if (native.getFrame(data, len, w, h)) {
        final length = len.value;
        final width = w.value;
        final height = h.value;
        if (length > 0 && width > 0 && height > 0 && data.value != nullptr) {
          final bytes = Uint8List.fromList(data.value.asTypedList(length));
          native.chronodeskFreeFrame(data.value);
          frameFreed = true;
          _decodeFrame(bytes, width, height);
        }
      }
    } catch (_) {
      if (!frameFreed && data.value != nullptr) {
        native.chronodeskFreeFrame(data.value);
      }
    } finally {
      calloc.free(data);
      calloc.free(len);
      calloc.free(w);
      calloc.free(h);
    }
  }

  void _handleFileTransferEvent(String json) {
    try {
      final map = jsonDecode(json) as Map<String, dynamic>;
      final type = map['type'] as String?;
      final id = map['id'] as String? ?? '';
      if (type == null || id.isEmpty) return;
      switch (type) {
        case 'file_request':
          final name = map['name'] as String? ?? 'unknown';
          final size = map['size'] as int? ?? 0;
          if (mounted) _showFileRequest(id, name, size);
        case 'file_progress':
          final bytesReceived = map['bytes_received'] as int?;
          final bytesSent = map['bytes_sent'] as int?;
          final totalSize = map['total_size'] as int? ?? 0;
          setState(() {
            _activeTransfers[id] = {
              'bytes': bytesReceived ?? bytesSent ?? 0,
              'total': totalSize,
            };
          });
        case 'file_complete':
          final name = map['name'] as String? ?? '';
          final path = map['path'] as String? ?? '';
          setState(() => _activeTransfers.remove(id));
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text('File received: $name'), action: SnackBarAction(label: 'Open', onPressed: () {
                if (path.isNotEmpty) Process.start('explorer', ['/select,', path]);
              })),
            );
          }
        case 'file_sent':
          final name = map['name'] as String? ?? '';
          final size = map['size'] as int? ?? 0;
          setState(() => _activeTransfers.remove(id));
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text('File sent: $name (${_formatSize(size)})')),
            );
          }
        case 'file_rejected':
          setState(() => _activeTransfers.remove(id));
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              const SnackBar(content: Text('File transfer rejected')),
            );
          }
        case 'file_cancelled':
          setState(() => _activeTransfers.remove(id));
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text('Transfer cancelled: ${map['name'] ?? ''}')),
            );
          }
        case 'file_error':
          final msg = map['msg'] as String? ?? 'unknown error';
          setState(() => _activeTransfers.remove(id));
          if (mounted) {
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(content: Text('File transfer error: $msg')),
            );
          }
      }
    } catch (e) {
      debugPrint('_handleFileTransferEvent error: $e');
    }
  }

  Future<void> _sendFile() async {
    final result = await FilePicker.platform.pickFiles();
    if (result == null || result.files.isEmpty) return;
    final path = result.files.first.path;
    if (path == null) return;
    final id = native.sendFile(path);
    if (id.isEmpty) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Failed to send file')),
        );
      }
      return;
    }
    final file = File(path);
    final int size;
    try {
      size = await file.length();
    } catch (_) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Cannot read file')),
        );
      }
      return;
    }
    if (!mounted) return;
    setState(() {
      _activeTransfers[id] = {
        'bytes': 0,
        'total': size,
        'name': result.files.first.name,
        'outgoing': true,
      };
    });
  }

  void _showFileRequest(String id, String name, int size) {
    showDialog(
      context: context,
      barrierDismissible: false,
      builder: (ctx) => AlertDialog(
        title: const Text('Incoming File'),
        content: Text('Receive file "$name" (${_formatSize(size)})?'),
        actions: [
          TextButton(
            onPressed: () {
              native.rejectFileTransfer(id);
              Navigator.of(ctx).pop();
            },
            child: const Text('Reject'),
          ),
          FilledButton(
            onPressed: () {
              native.acceptFileTransfer(id);
              setState(() {
                _activeTransfers[id] = {'bytes': 0, 'total': size, 'name': name};
              });
              Navigator.of(ctx).pop();
            },
            child: const Text('Accept'),
          ),
        ],
      ),
    );
  }

  String _formatSize(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
    return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }

  Future<void> _decodeFrame(Uint8List rgba, int w, int h) async {
    final completer = Completer<ui.Image>();
    ui.decodeImageFromPixels(rgba, w, h, ui.PixelFormat.rgba8888, (img) {
      completer.complete(img);
    });
    final img = await completer.future;
    if (!mounted) return;
    setState(() {
      _frameImage?.dispose();
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
    malloc.free(ptr);
  }

  void _disconnect() {
    native.chronodeskDisconnect();
  }

  Future<void> _exportLogs() async {
    try {
      final log = native.getLog();
      if (log.isEmpty) return;
      final desktop = Platform.environment['USERPROFILE'] != null
          ? '${Platform.environment['USERPROFILE']}\\Desktop'
          : (await getApplicationDocumentsDirectory()).path;
      final file = File('$desktop\\chronodesk_crash_${DateTime.now().millisecondsSinceEpoch}.log');
      await file.writeAsString(log);
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Log saved to ${file.path}')),
        );
      }
    } catch (e) {
      debugPrint('_exportLogs error: $e');
    }
  }

  void _showSettings() {
    _addrController.text = native.getConfig('signaling_addr');
    if (_addrController.text.isEmpty) _addrController.text = '82.70.239.217:21116';
    _turnUrlController.text = native.getConfig('turn_url');
    _turnUserController.text = native.getConfig('turn_username');
    _turnPassController.text = native.getConfig('turn_password');
    _downloadDirController.text = native.getConfig('download_dir');
    showDialog(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Settings'),
        content: SingleChildScrollView(
          child: Column(
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
              const Text('TURN Server (optional)', style: TextStyle(fontSize: 13, color: Colors.grey)),
              const SizedBox(height: 8),
              TextField(
                controller: _turnUrlController,
                style: const TextStyle(fontSize: 14),
                decoration: InputDecoration(
                  hintText: 'turn:host:3478',
                  filled: true,
                  fillColor: Colors.grey.shade900,
                  border: OutlineInputBorder(borderRadius: BorderRadius.circular(8)),
                  contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                ),
              ),
              const SizedBox(height: 8),
              Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: _turnUserController,
                      style: const TextStyle(fontSize: 14),
                      decoration: InputDecoration(
                        hintText: 'Username',
                        filled: true,
                        fillColor: Colors.grey.shade900,
                        border: OutlineInputBorder(borderRadius: BorderRadius.circular(8)),
                        contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: TextField(
                      controller: _turnPassController,
                      style: const TextStyle(fontSize: 14),
                      obscureText: true,
                      decoration: InputDecoration(
                        hintText: 'Password',
                        filled: true,
                        fillColor: Colors.grey.shade900,
                        border: OutlineInputBorder(borderRadius: BorderRadius.circular(8)),
                        contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Text(
                'Restart app after changing TURN settings.',
                style: TextStyle(fontSize: 12, color: Colors.grey.shade500),
              ),
              const SizedBox(height: 24),
              const Text('Download Directory', style: TextStyle(fontSize: 13, color: Colors.grey)),
              const SizedBox(height: 8),
              Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: _downloadDirController,
                      style: const TextStyle(fontSize: 14),
                      decoration: InputDecoration(
                        hintText: 'Leave empty for system temp',
                        filled: true,
                        fillColor: Colors.grey.shade900,
                        border: OutlineInputBorder(borderRadius: BorderRadius.circular(8)),
                        contentPadding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
                      ),
                    ),
                  ),
                  const SizedBox(width: 8),
                  IconButton(
                    icon: const Icon(Icons.folder_open),
                    onPressed: () async {
                      final dir = await FilePicker.platform.getDirectoryPath();
                      if (dir != null) {
                        _downloadDirController.text = dir;
                      }
                    },
                  ),
                ],
              ),
              const SizedBox(height: 24),
              const Divider(color: Colors.grey),
              const SizedBox(height: 8),
              TextButton.icon(
                onPressed: () => _checkUpdate(ctx),
                icon: const Icon(Icons.system_update, size: 18),
                label: const Text('Check for Updates'),
              ),
              const SizedBox(height: 4),
              TextButton.icon(
                onPressed: () {
                  _exportLogs().catchError((_) {});
                },
                icon: const Icon(Icons.bug_report, size: 18),
                label: const Text('Export Crash Logs'),
              ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Close'),
          ),
          FilledButton(
            onPressed: () {
              native.setConfig('signaling_addr', _addrController.text.trim());
              native.setConfig('turn_url', _turnUrlController.text.trim());
              native.setConfig('turn_username', _turnUserController.text.trim());
              native.setConfig('turn_password', _turnPassController.text.trim());
              native.setConfig('download_dir', _downloadDirController.text.trim());
              setState(() => _signalingAddr = _addrController.text.trim());
              Navigator.of(ctx).pop();
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(content: Text('Settings saved. Restart to apply.')),
              );
            },
            child: const Text('Save'),
          ),
        ],
      ),
    );
  }

  void _autoCheckUpdate() {
    if (_updateDialogOpen) return;
    checkForUpdate().then((update) {
      if (!mounted) return;
      if (update == null) {
        _updateAvailable = false;
        if (mounted) setState(() {});
        return;
      }
      _updateAvailable = true;
      if (mounted) setState(() {});
      _updateDialogOpen = true;
      try {
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
              TextButton(onPressed: () {
                _updateDialogOpen = false;
                Navigator.of(ctx).pop();
              }, child: const Text('Later')),
              FilledButton.icon(
                onPressed: () {
                  _updateDialogOpen = false;
                  Navigator.of(ctx).pop();
                  _performUpdate();
                },
                icon: const Icon(Icons.download, size: 18),
                label: const Text('Update'),
              ),
            ],
          ),
        );
      } catch (_) {
        _updateDialogOpen = false;
      }
    }).catchError((_) {
      _updateAvailable = false;
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
      if (mounted) Navigator.of(settingsCtx).pop();
      if (update == null) {
        if (mounted) {
          showDialog(
            context: context,
            builder: (ctx) => AlertDialog(
              title: const Text('Up to Date'),
              content: Text('CHRONODESK v$currentVersion is the latest version.'),
              actions: [FilledButton(onPressed: () => Navigator.of(ctx).pop(), child: const Text('OK'))],
            ),
          );
        }
        return;
      }
      if (!mounted) return;
      _updateAvailable = true;
      if (mounted) setState(() {});
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
                _performUpdate();
              },
              icon: const Icon(Icons.download, size: 18),
              label: const Text('Download Update'),
            ),
          ],
        ),
      );
    }).catchError((_) {
      if (mounted) {
        try {
          Navigator.of(settingsCtx, rootNavigator: true).pop();
        } catch (_) {}
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Failed to check for updates. Check your internet connection.')),
        );
      }
    });
  }

  void _performUpdate() {
    if (_isUpdating) return;
    _isUpdating = true;
    bool cancelled = false;
    final progressState = ValueNotifier<double>(0.0);
    final progressText = ValueNotifier<String>('Downloading...');
    BuildContext? dialogContext;
    showDialog(
      context: context,
      barrierDismissible: false,
      builder: (ctx) {
        dialogContext = ctx;
        return AlertDialog(
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
                    ValueListenableBuilder<String>(
                      valueListenable: progressText,
                      builder: (ctx, text, _) => Text(text),
                    ),
                  ],
                ),
                if (progress > 0) ...[
                  const SizedBox(height: 8),
                  LinearProgressIndicator(value: progress),
                ],
                const SizedBox(height: 16),
                TextButton.icon(
                  onPressed: () {
                    cancelled = true;
                    cancelUpdate();
                    if (dialogContext != null && dialogContext!.mounted) {
                      Navigator.of(dialogContext!).pop();
                    }
                  },
                  icon: const Icon(Icons.cancel, size: 16),
                  label: const Text('Cancel'),
                ),
              ],
            ),
          ),
        );
      },
    );
    downloadAndApplyUpdate((received, total) {
      if (cancelled) return;
      if (total > 0) {
        progressState.value = received / total;
        progressText.value = '${(received / total * 100).toInt()}%';
      } else {
        progressText.value = '${(received ~/ 1024)} KB downloaded';
      }
    }).then((_) {
      _isUpdating = false;
    }).catchError((e) {
      if (cancelled) {
        _isUpdating = false;
        return;
      }
      _isUpdating = false;
      if (mounted) {
        try {
          if (dialogContext != null && dialogContext!.mounted) {
            Navigator.of(dialogContext!).pop();
          }
        } catch (_) {}
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
          Badge(
            isLabelVisible: _updateAvailable,
            label: const Icon(Icons.system_update, size: 12),
            child: IconButton(
              icon: Icon(Icons.settings, color: Colors.grey.shade400),
              onPressed: _showSettings,
              tooltip: 'Settings',
            ),
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
          if (_updateAvailable) ...[
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
              decoration: BoxDecoration(
                color: Colors.cyan.withAlpha(25),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: Colors.cyan.shade700),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(Icons.system_update, size: 18, color: Colors.cyan.shade300),
                  const SizedBox(width: 8),
                  Text('Update available — Open Settings',
                    style: TextStyle(fontSize: 13, color: Colors.cyan.shade200)),
                ],
              ),
            ),
            const SizedBox(height: 24),
          ],
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
            const SizedBox(height: 12),
            SizedBox(
              width: 200,
              child: OutlinedButton.icon(
                onPressed: _sendFile,
                icon: const Icon(Icons.upload_file),
                label: const Text('Send File'),
                style: OutlinedButton.styleFrom(
                  foregroundColor: Colors.blueGrey.shade300,
                  side: BorderSide(color: Colors.blueGrey.shade700),
                  padding: const EdgeInsets.all(14),
                ),
              ),
            ),
            if (_activeTransfers.isNotEmpty) ...[
              const SizedBox(height: 24),
              ..._buildTransferProgress(),
            ],
          ],
        ],
      ),
    );
  }

  List<Widget> _buildTransferProgress() {
    return _activeTransfers.entries.map((e) {
      final data = e.value;
      final bytes = data['bytes'] as int? ?? 0;
      final total = data['total'] as int? ?? 0;
      final name = data['name'] as String? ?? e.key;
      final progress = total > 0 ? bytes / total : 0.0;
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(child: Text(name, style: const TextStyle(color: Colors.white, fontSize: 13))),
                SizedBox(width: 8),
                SizedBox(
                  height: 24,
                  child: TextButton.icon(
                    onPressed: () => native.cancelFileTransfer(e.key),
                    icon: Icon(Icons.close, size: 14, color: Colors.red.shade300),
                    label: Text('Cancel', style: TextStyle(fontSize: 11, color: Colors.red.shade300)),
                    style: TextButton.styleFrom(padding: const EdgeInsets.symmetric(horizontal: 6), minimumSize: Size.zero, tapTargetSize: MaterialTapTargetSize.shrinkWrap),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 4),
            Row(
              children: [
                Expanded(
                  child: ClipRRect(
                    borderRadius: BorderRadius.circular(4),
                    child: LinearProgressIndicator(value: progress, minHeight: 8, backgroundColor: Colors.white12),
                  ),
                ),
                const SizedBox(width: 8),
                Text('${(progress * 100).toStringAsFixed(0)}%', style: const TextStyle(color: Colors.white70, fontSize: 12)),
              ],
            ),
          ],
        ),
      );
    }).toList();
  }

  Offset _viewportToImage(Offset viewportPos) {
    final matrix = _transformController.value;
    final inv = Matrix4.inverted(matrix);
    final transformed = MatrixUtils.transformPoint(inv, viewportPos);
    return transformed;
  }

  void _onPointerMove(PointerMoveEvent e) {
    if (_frameW <= 0 || _frameH <= 0) return;
    final img = _viewportToImage(e.localPosition);
    native.chronodeskSendInputMove(
      img.dx.round().clamp(0, _frameW - 1),
      img.dy.round().clamp(0, _frameH - 1),
    );
  }

  void _onPointerDown(PointerDownEvent e) {
    if (_frameW > 0 && _frameH > 0) {
      final img = _viewportToImage(e.localPosition);
      native.chronodeskSendInputMove(
        img.dx.round().clamp(0, _frameW - 1),
        img.dy.round().clamp(0, _frameH - 1),
      );
    }
    native.chronodeskSendInputClick(1, true);
  }

  void _onPointerUp(PointerUpEvent e) {
    native.chronodeskSendInputClick(1, false);
  }

  Widget _buildRemoteView() {
    return Focus(
      autofocus: true,
      onKeyEvent: (_, event) {
        final isDown = event is KeyDownEvent || event is KeyRepeatEvent;
        final isUp = event is KeyUpEvent;
        final logical = event.logicalKey;

        if (isDown && logical == LogicalKeyboardKey.escape) {
          setState(() => _captureKeyboard = !_captureKeyboard);
          return KeyEventResult.handled;
        }
        if (isDown && (_captureKeyboard && (logical == LogicalKeyboardKey.tab ||
            logical == LogicalKeyboardKey.altLeft || logical == LogicalKeyboardKey.altRight ||
            logical == LogicalKeyboardKey.metaLeft || logical == LogicalKeyboardKey.metaRight))) {
          return KeyEventResult.ignored;
        }
        if (_captureKeyboard && (isDown || isUp)) {
          native.chronodeskSendInputKey(logical.keyId, isDown);
          return KeyEventResult.handled;
        }
        return KeyEventResult.ignored;
      },
      child: Column(
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
                  if (_rttMs > 0)
                    Padding(
                      padding: const EdgeInsets.only(right: 12),
                      child: Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(Icons.speed, size: 14, color: _rttMs > 200 ? Colors.red.shade300 : Colors.green.shade400),
                          const SizedBox(width: 4),
                          Text('${_rttMs}ms', style: TextStyle(fontSize: 12, color: Colors.grey.shade400)),
                          const SizedBox(width: 8),
                          Text('Q$_qualityLevel', style: TextStyle(fontSize: 12, color: Colors.grey.shade400)),
                          const SizedBox(width: 8),
                          Text('${_targetFps}fps', style: TextStyle(fontSize: 12, color: Colors.grey.shade400)),
                        ],
                      ),
                    ),
                  IconButton(
                    icon: Icon(
                      _audioMuted ? Icons.volume_off : Icons.volume_up,
                      color: _audioMuted ? Colors.red.shade300 : Colors.green.shade400,
                      size: 20,
                    ),
                    onPressed: () => setState(() => _audioMuted = !_audioMuted),
                    tooltip: _audioMuted ? 'Unmute audio' : 'Mute audio',
                  ),
                  if (_connected && !_isHost)
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                      decoration: BoxDecoration(
                        color: (_captureKeyboard ? Colors.green : Colors.red).withAlpha(51),
                        borderRadius: BorderRadius.circular(4),
                        border: Border.all(color: _captureKeyboard ? Colors.green : Colors.red, width: 0.5),
                      ),
                      child: Text(_captureKeyboard ? 'KBD' : 'KBD OFF', style: TextStyle(fontSize: 10, color: _captureKeyboard ? Colors.green : Colors.red)),
                    ),
                  IconButton(
                    icon: const Icon(Icons.upload_file, color: Colors.white70, size: 20),
                    onPressed: _sendFile,
                    tooltip: 'Send file',
                  ),
                  IconButton(
                    icon: Icon(Icons.close, color: Colors.red.shade300),
                    onPressed: _disconnect,
                    tooltip: 'Disconnect',
                  ),
                ],
              ),
            ),
          ),
          if (_activeTransfers.isNotEmpty)
            Container(
              color: const Color(0xFF0F0F1A),
              width: double.infinity,
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: _buildTransferProgress(),
              ),
            ),
          Expanded(
            child: Listener(
              onPointerMove: _onPointerMove,
              onPointerDown: _onPointerDown,
              onPointerUp: _onPointerUp,
              child: InteractiveViewer(
                transformationController: _transformController,
                constrained: false,
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
      ),
    );
  }
}
