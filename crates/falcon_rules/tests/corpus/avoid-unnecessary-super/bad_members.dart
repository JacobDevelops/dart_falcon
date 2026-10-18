class Base {
  int value = 0;
  int get x => 1;
  set x(int value) {}
  int method(int value) => value;
  void action() {}
}
final class Child extends Base {
  int read() => super.value; /* expect: avoid-unnecessary-super */
  int get y => super.x; /* expect: avoid-unnecessary-super */
  int invoke() => super.method(2); /* expect: avoid-unnecessary-super */
  void run() { super.action(); } /* expect: avoid-unnecessary-super */
  void write(int value) { super.x = value; } /* expect: avoid-unnecessary-super */
}
