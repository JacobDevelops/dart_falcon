import 'package:riverpod/riverpod.dart';

class Counter extends Notifier<int> {
  Counter();
  int build() => 1;
}

class EmptyBody extends AsyncNotifier<int> {
  EmptyBody() { /* No initialization logic. */ }
}

class NoConstructor extends StreamNotifier<int> {}

class EmptyNamed extends AutoDisposeNotifier<int> {
  EmptyNamed.create() {}
}

class OrdinaryClass {
  OrdinaryClass() { initialize(); }
  void initialize() {}
}
