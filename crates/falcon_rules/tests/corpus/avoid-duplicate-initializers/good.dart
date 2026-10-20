void changed(int input) {
  final first = input;
  input = 2;
  final second = input;
}
void effects(dynamic object) {
  final first = object.value;
  object.update();
  final second = object.value;
}
void calls(dynamic object) {
  final first = object.read();
  final second = object.read();
}
void allocations() {
  final first = Object();
  final second = Object();
  final third = <int>[];
  final fourth = <int>[];
}
void scopes(int input) {
  if (input > 0) { final first = input; }
  if (input < 0) { final second = input; }
}
void distinct(dynamic object) {
  final first = object.first;
  final second = object.second;
  final text = 'a b';
  final other = 'ab';
}
void mutable(int input) {
  var first = input;
  var second = input;
}
class A {
  final first = Object();
  final second = Object();
}
class B {
  final first = 1;
  static final second = 1;
}
int current = 0;
final firstSnapshot = current;
final secondSnapshot = current;
class LazySnapshots {
  static final first = current;
  static final second = current;
  static final incremented = current + 1;
  static final incrementedAgain = current + 1;
  static final text = 'value: $current';
  static final textAgain = 'value: $current';
}
