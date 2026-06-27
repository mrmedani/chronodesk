import 'package:flutter/material.dart';

void main() {
  runApp(const ChronodeskApp());
}

class ChronodeskApp extends StatelessWidget {
  const ChronodeskApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'CHRONODESK',
      theme: ThemeData.dark(),
      home: const HomeScreen(),
    );
  }
}

class HomeScreen extends StatelessWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('CHRONODESK')),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Text('Connect to remote desktop'),
            const SizedBox(height: 20),
            ElevatedButton(
              onPressed: () {},
              child: const Text('New Connection'),
            ),
          ],
        ),
      ),
    );
  }
}
