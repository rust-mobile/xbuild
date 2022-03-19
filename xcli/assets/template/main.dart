import 'package:flutter/material.dart';
import './bindings.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'FlutterDemo',
      home: CounterPage(state: Api.load().createCounterState()),
    );
  }
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
                return Text('$counter', style: Theme.of(context).textTheme.headline4);
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
