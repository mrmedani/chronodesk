import 'package:flutter/material.dart';

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
            const Icon(Icons.desktop_windows, size: 80, color: Colors.blueGrey),
            const SizedBox(height: 24),
            const Text(
              'Remote Desktop',
              style: TextStyle(fontSize: 28, fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 32),
            SizedBox(
              width: 240,
              child: ElevatedButton.icon(
                onPressed: () => Navigator.pushNamed(context, '/host'),
                icon: const Icon(Icons.cast),
                label: const Text('Host'),
                style: ElevatedButton.styleFrom(
                  padding: const EdgeInsets.all(16),
                ),
              ),
            ),
            const SizedBox(height: 16),
            SizedBox(
              width: 240,
              child: ElevatedButton.icon(
                onPressed: () => Navigator.pushNamed(context, '/viewer'),
                icon: const Icon(Icons.tv),
                label: const Text('Connect to'),
                style: ElevatedButton.styleFrom(
                  padding: const EdgeInsets.all(16),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
