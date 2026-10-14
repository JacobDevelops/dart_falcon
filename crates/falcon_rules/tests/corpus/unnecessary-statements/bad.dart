int topLevel = 0;

void topFunction() {}

int methodOne() => 1;

int methodTwo() => 2;

class Box {
  int value = 0;
  static int count = 0;

  void tick() {}

  void run(List<int> list, bool foo, int a, int b) {
    value; /* expect: unnecessary-statements */
    this.value; /* expect: unnecessary-statements */
    tick; /* expect: unnecessary-statements */
    Box.count; /* expect: unnecessary-statements */
    list..clear; /* expect: unnecessary-statements */
  }
}

void f(List<int> list, bool foo, int a, int b, Box box) {
  a; /* expect: unnecessary-statements */
  list.clear; /* expect: unnecessary-statements */
  1 + 2; /* expect: unnecessary-statements */
  methodOne() + methodTwo(); /* expect: unnecessary-statements */
  foo ? a : b; /* expect: unnecessary-statements */ /* expect: unnecessary-statements */
  topLevel; /* expect: unnecessary-statements */
  topFunction; /* expect: unnecessary-statements */
  box.value; /* expect: unnecessary-statements */
  box.tick; /* expect: unnecessary-statements */
  'string'; /* expect: unnecessary-statements */
  list[0]; /* expect: unnecessary-statements */
  a is int; /* expect: unnecessary-statements */
  -a; /* expect: unnecessary-statements */
  [1, 2]; /* expect: unnecessary-statements */
  foo && a > 0; /* expect: unnecessary-statements */
  (methodOne()); /* expect: unnecessary-statements */
  Box; /* expect: unnecessary-statements */
  for (a; a < 10; a + 1) {} /* expect: unnecessary-statements */ /* expect: unnecessary-statements */
}
