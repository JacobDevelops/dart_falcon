// Initializing formals and super parameters without a redundant type.
class A {
  int x;
  double y;
  A(this.x, this.y);
}

class Base {
  final int a;
  Base(this.a);
}

class Derived extends Base {
  Derived(super.a);
}

class C {
  String name;
  num size;
  C(this.name);
  C.sized(this.size);
  void use() {
    print(this.name);
    return this.size.isNaN ? null : null;
  }
}

class D {
  bool flag;
  D(this.flag);
}

// A *narrowing* type annotation on an initializing formal is not redundant: it
// restricts the constructor's parameter below the field's declared type.
class Narrowing {
  num x;
  Object label;
  Narrowing(int this.x);
  Narrowing.labelled(String this.label);
}

// The same narrowing applies to super parameters.
class WideBase {
  final num a;
  final Object b;
  WideBase(this.a, {required this.b});
}

class NarrowDerived extends WideBase {
  NarrowDerived(int super.a, {required String super.b});
}

// A pattern guard puts `this.` right after a type-like token without any
// initializing formal being involved.
class Guarded {
  bool flag;
  Guarded(this.flag);
  String describe(Object value) {
    switch (value) {
      case String s when this.flag:
        return s;
      case int i when this.flag:
        return '$i';
      default:
        return '';
    }
  }
}
