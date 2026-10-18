import 'package:flutter/widgets.dart';

abstract class WidgetRef {
  Object? watch(Object provider);
  Object? read(Object provider);
}

abstract class ConsumerWidget extends Widget {
  const ConsumerWidget();
  Widget build(BuildContext context, WidgetRef ref);
}

class Consumer extends Widget {
  const Consumer({required this.builder});
  final Widget Function(BuildContext context, WidgetRef ref, Widget? child) builder;
}
