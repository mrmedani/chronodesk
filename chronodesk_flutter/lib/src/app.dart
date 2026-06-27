import 'package:flutter/material.dart';
import 'screens/home_screen.dart';
import 'screens/host_screen.dart';
import 'screens/viewer_screen.dart';

class ChronodeskApp extends StatelessWidget {
  const ChronodeskApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'CHRONODESK',
      theme: ThemeData(
        brightness: Brightness.dark,
        primarySwatch: Colors.blueGrey,
        useMaterial3: true,
      ),
      initialRoute: '/',
      routes: {
        '/': (_) => const HomeScreen(),
        '/host': (_) => const HostScreen(),
        '/viewer': (_) => const ViewerScreen(),
      },
    );
  }
}
