import 'package:flutter/material.dart';
import 'src/app.dart';
import 'src/ffi/native.dart' as native;

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  try {
    native.ensureInitialized();
  } catch (e) {
    runApp(MaterialApp(
      home: Scaffold(
        backgroundColor: const Color(0xFF1A1A2E),
        body: Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Text(
              'Failed to initialize: $e',
              style: const TextStyle(color: Colors.white, fontSize: 16),
              textAlign: TextAlign.center,
            ),
          ),
        ),
      ),
    ));
    return;
  }
  runApp(const ChronodeskApp());
}
