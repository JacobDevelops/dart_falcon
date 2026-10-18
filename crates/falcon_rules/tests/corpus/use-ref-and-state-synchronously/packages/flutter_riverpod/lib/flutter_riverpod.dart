import 'package:flutter/widgets.dart';
abstract class WidgetRef { T read<T>(Object provider); }
abstract class Ref { bool get mounted; T read<T>(Object provider); }
abstract class ConsumerState<T> extends State<T> { WidgetRef get ref; }
abstract class AnyNotifier<T> { Ref get ref; T get state; set state(T value); }
abstract class Notifier<T> extends AnyNotifier<T> {}
abstract class AsyncNotifier<T> extends Notifier<T> {}
abstract class StreamNotifier<T> extends Notifier<T> {}
abstract class BuildlessNotifier<T> { Ref get ref; T get state; set state(T value); }
