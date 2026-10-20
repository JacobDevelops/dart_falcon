import 'package:riverpod/riverpod.dart' as rp;
abstract class Good extends rp.Notifier<int> {
  int _count = 0;
  int get _current => _count;
  set _current(int value) { _count = value; }
  static int cache = 0;
  int build() => _count;
  void increment() { state = state + 1; }
}
class Notifier<T> {}
class Unrelated extends Notifier<int> { int publicValue = 0; }
class Plain { int value = 0; int get other => value; }
