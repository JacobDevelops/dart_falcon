void topLevel() {}
int anotherTopLevel() => 1;
class Example {
  void method() {}
  int get value => 1;
  set value(int value) {}
  void callbacks() {
    final callback = () {};
    final int Function(int) identity = (value) => value;
    [1, 2].forEach((value) {});
  }
}
