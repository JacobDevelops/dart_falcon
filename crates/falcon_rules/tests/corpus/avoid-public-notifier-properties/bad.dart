import 'package:riverpod/riverpod.dart';
abstract class Counter extends Notifier<int> {
  int count = 0; /* expect: avoid-public-notifier-properties */
  final int initial = 0; /* expect: avoid-public-notifier-properties */
  int get current => count; /* expect: avoid-public-notifier-properties */
  set current(int value) { count = value; } /* expect: avoid-public-notifier-properties */
}
abstract class AsyncCounter extends AsyncNotifier<int> {
  int cached = 0; /* expect: avoid-public-notifier-properties */
}
abstract class Derived extends Counter {
  int extra = 0; /* expect: avoid-public-notifier-properties */
}
