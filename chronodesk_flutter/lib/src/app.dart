import 'package:flutter/material.dart';
import 'screens/home_screen.dart';

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
      home: const HomeScreen(),
    );
  }
}
