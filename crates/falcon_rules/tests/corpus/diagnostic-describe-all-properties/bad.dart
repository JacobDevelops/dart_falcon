import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

class Tile extends StatelessWidget {
  const Tile({
    super.key,
    required this.title,
    required this.count,
    required this.tags,
    required this.child,
  });

  final String title; /* expect: diagnostic-describe-all-properties */
  final int count;
  final List<String> tags; /* expect: diagnostic-describe-all-properties */
  final Widget child;

  bool get enabled => count > 0; /* expect: diagnostic-describe-all-properties */

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    super.debugFillProperties(properties);
    properties.add(IntProperty('count', count));
  }

  @override
  Widget build(BuildContext context) => child;
}

class Counter extends StatefulWidget {
  const Counter({super.key});

  @override
  State<Counter> createState() => _CounterState();
}

class _CounterState extends State<Counter> {
  int clicks = 0; /* expect: diagnostic-describe-all-properties */
  DateTime? lastClick; /* expect: diagnostic-describe-all-properties */

  @override
  Widget build(BuildContext context) => const Placeholder();
}

class Model with Diagnosticable {
  Model(this.value);

  final double value; /* expect: diagnostic-describe-all-properties */
  final Duration timeout = Duration.zero; /* expect: diagnostic-describe-all-properties */
}

class Choice<T> extends StatelessWidget {
  const Choice({super.key, required this.value});

  final T value; /* expect: diagnostic-describe-all-properties */

  @override
  Widget build(BuildContext context) => const SizedBox();
}

abstract class SinkWidget extends StatelessWidget {
  const SinkWidget({super.key});

  set sink(int value);
}

class Sink extends SinkWidget {
  Sink({super.key});

  // A field does not override an inherited setter.
  int sink = 0; /* expect: diagnostic-describe-all-properties */

  @override
  Widget build(BuildContext context) => const SizedBox();
}

class InferredModel with Diagnosticable {
  final count = 1; /* expect: diagnostic-describe-all-properties */
  final values = <String>[]; /* expect: diagnostic-describe-all-properties */
}

class DebugScopeModel with Diagnosticable {
  final int outsideBody = 1; /* expect: diagnostic-describe-all-properties */
  final int nestedReference = 2;

  void otherMethod() {
    print(outsideBody);
  }

  @override
  void debugFillProperties(DiagnosticPropertiesBuilder properties) {
    final deferred = () => nestedReference;
    properties.add(IntProperty('nestedReference', deferred()));
  }
}
