import 'package:riverpod/riverpod.dart';

class Counter extends Notifier<int> {
  Counter() { state = 1; } /* expect: avoid-notifier-constructors */
  int build() => 0;
}

class InitializedCounter extends Notifier<int> {
  final int initial;
  InitializedCounter() : initial = 1; /* expect: avoid-notifier-constructors */
  int build() => initial;
}
