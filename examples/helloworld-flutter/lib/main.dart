import 'package:flutter/material.dart';
import 'package:nativeshell/nativeshell.dart';

void main() async {
  runApp(MyApp());
}

class MyApp extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      home: DefaultTextStyle(
        style: TextStyle(
          color: Colors.white,
          fontSize: 14,
        ),
        child: Container(
          color: Colors.black,
          child: WindowWidget(
            onCreateState: (initData) {
              WindowState? state;
              state ??= MainWindowState();
              return state;
            },
          ),
        ),
      ),
    );
  }
}

class MainWindowState extends WindowState {
  @override
  WindowSizingMode get windowSizingMode =>
      WindowSizingMode.atLeastIntrinsicSize;

  @override
  Widget build(BuildContext context) {
    return WindowLayoutProbe(
      child: Container(
        padding: EdgeInsets.all(20),
        child: Center(child: Text('Welcome to NativeShell!')),
      ),
    );
  }
}
