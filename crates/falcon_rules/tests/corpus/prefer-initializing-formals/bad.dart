// Parameters copied verbatim into same-named fields.
class A {
  int x;
  A(int x) : x = x; /* expect: prefer-initializing-formals */
}

class B {
  int y;
  B(int y) { this.y = y; } /* expect: prefer-initializing-formals */
}

class C {
  int a, b;
  C(int a, int b) : a = a, b = b; /* expect: prefer-initializing-formals */ /* expect: prefer-initializing-formals */
}

class D {
  String name;
  D(String name) : name = name; /* expect: prefer-initializing-formals */
}

class E {
  int v;
  E(int v) { this.v = v; } /* expect: prefer-initializing-formals */
}


class PrivateNamed {
  int _value;
  PrivateNamed({required int value}) : _value = value; /* expect: prefer-initializing-formals */
}

class Primary(int value) { 
  final int value;
  this : value = value; /* expect: prefer-initializing-formals */
}

class ParenthesizedBody {
  int value;
  ParenthesizedBody(int value) { this.value = (value); } /* expect: prefer-initializing-formals */
}

class Subtype {
  num value;
  Subtype(int value) : value = value; /* expect: prefer-initializing-formals */
}

class Compound {
  int value = 0;
  Compound(int value) { this.value += value; } /* expect: prefer-initializing-formals */
}
