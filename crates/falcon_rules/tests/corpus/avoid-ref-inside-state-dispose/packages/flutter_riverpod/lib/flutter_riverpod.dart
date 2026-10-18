export 'package:riverpod/riverpod.dart';
import 'package:riverpod/riverpod.dart';
abstract class WidgetRef implements Ref {}
abstract class ConsumerState<T> { WidgetRef get ref; void dispose() {} }
