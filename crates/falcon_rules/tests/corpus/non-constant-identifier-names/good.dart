// lowerCamelCase non-constant identifiers.

int myVar = 0;

void foo() {}

void f(int goodParam) {}

class A {
  void someMethod() {}

  A.myNamed();
}

void g() {
  var localVar = 1;
  var _private = 2;
  print(localVar + _private);
}

void h(int _) {}
