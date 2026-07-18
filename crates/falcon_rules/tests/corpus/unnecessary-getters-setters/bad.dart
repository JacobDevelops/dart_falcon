// Trivial getter/setter pairs that merely expose a private field.
class A {
  int _value = 0;
  int get value => _value; /* expect: unnecessary-getters-setters */
  set value(int v) => _value = v;
}

class B {
  String _name = '';
  String get name { return _name; } /* expect: unnecessary-getters-setters */
  set name(String v) { _name = v; }
}

class C {
  bool _flag = false;
  bool get flag => _flag; /* expect: unnecessary-getters-setters */
  set flag(bool v) { _flag = v; }
}

class D {
  double _size = 0;
  double get size => this._size; /* expect: unnecessary-getters-setters */
  set size(double v) { this._size = v; }
}

class E {
  num _count = 0;
  num get count => _count; /* expect: unnecessary-getters-setters */
  set count(num v) => _count = v;
}
