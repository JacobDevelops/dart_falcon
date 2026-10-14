void main() {}

int get answer => 42;

typedef void Handler(int x);

typedef Callback = Function(int);

set value(int v) {}

class Foo {
  int bar() => 1;
  static Foo make() => Foo();
  int get size => 0;
  set size(int v) {}
  Foo operator +(Foo other) => this;
  operator []=(int index, int value) {}
}

void outer() {
  void inner() {}
  inner();
  final f = () => 1;
  f();
}
