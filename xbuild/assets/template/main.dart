import 'dart:io' show Platform;
import 'package:flutter/material.dart';
import 'package:nativeshell/nativeshell.dart';
import './bindings.dart';

void main() {
  final title = 'FlutterDemo';
  final home = CounterPage(state: Api.load().createCounterState());
  final app = Platform.isAndroid || Platform.isIOS
      ? MaterialApp(
          title: title,
          home: home,
        )
      : WindowWidget(onCreateState: (initData) {
          WindowState? state;
          state ??= MainWindow(
            title: title,
            home: home,
          );
          return state;
        });
  runApp(app);
}

class CounterPage extends StatelessWidget {
  const CounterPage({Key? key, required this.state}) : super(key: key);

  final CounterState state;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text('counter page'),
      ),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const Text('You have pushed the button this many times:'),
            StreamBuilder(
              stream: state.subscribe(),
              builder: (BuildContext context, AsyncSnapshot snapshot) {
                final counter = state.counter();
                return Text('$counter',
                    style: Theme.of(context).textTheme.headline4);
              },
            ),
          ],
        ),
      ),
      floatingActionButton: FloatingActionButton(
        onPressed: state.increment,
        tooltip: 'Increment',
        child: const Icon(Icons.add),
      ),
    );
  }
}

class MainWindow extends WindowState {
  MainWindow({required this.title, required this.home});

  final String title;
  final Widget home;

  @override
  WindowSizingMode get windowSizingMode =>
      WindowSizingMode.atLeastIntrinsicSize;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: title,
      home: WindowLayoutProbe(
        child: home,
      ),
    );
  }
}
