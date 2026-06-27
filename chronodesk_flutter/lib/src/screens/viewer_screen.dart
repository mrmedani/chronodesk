import 'package:flutter/material.dart';

class ViewerScreen extends StatefulWidget {
  const ViewerScreen({super.key});

  @override
  State<ViewerScreen> createState() => _ViewerScreenState();
}

class _ViewerScreenState extends State<ViewerScreen> {
  final _peerIdController = TextEditingController();
  String _status = '';
  bool _connected = false;

  @override
  void dispose() {
    _peerIdController.dispose();
    super.dispose();
  }

  void _connect() {
    final peerId = _peerIdController.text.trim();
    if (peerId.isEmpty) return;
    setState(() {
      _status = 'Connecting to $peerId...';
    });
    // TODO: initiate WebRTC connection via FFI
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Connect to Host')),
      body: Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          children: [
            TextField(
              controller: _peerIdController,
              decoration: const InputDecoration(
                labelText: 'Host Peer ID',
                border: OutlineInputBorder(),
                prefixIcon: Icon(Icons.key),
              ),
              style: const TextStyle(fontSize: 20),
              textAlign: TextAlign.center,
            ),
            const SizedBox(height: 24),
            SizedBox(
              width: double.infinity,
              child: FilledButton.icon(
                onPressed: _connected ? null : _connect,
                icon: const Icon(Icons.link),
                label: Text(_connected ? 'Connected' : 'Connect'),
                style: FilledButton.styleFrom(
                  padding: const EdgeInsets.all(16),
                ),
              ),
            ),
            if (_status.isNotEmpty) ...[
              const SizedBox(height: 16),
              Text(_status, style: const TextStyle(color: Colors.grey)),
            ],
            const Spacer(),
            if (_connected)
              Container(
                width: double.infinity,
                height: 300,
                decoration: BoxDecoration(
                  color: Colors.black,
                  borderRadius: BorderRadius.circular(8),
                ),
                child: const Center(
                  child: Text(
                    'Remote Screen',
                    style: TextStyle(color: Colors.white54),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}
