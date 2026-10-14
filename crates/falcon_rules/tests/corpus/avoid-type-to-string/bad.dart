void foo(String Function() f) {}

var fromRuntimeType = 7.runtimeType.toString(); /* expect: avoid-type-to-string */

void tearOff() {
  foo(7.runtimeType.toString); /* expect: avoid-type-to-string */
}

class Type2 implements Type {}

class Type3 extends Type2 {}

var direct = Type2().toString(); /* expect: avoid-type-to-string */

var inherited = Type3().toString(); /* expect: avoid-type-to-string */

class Type4 implements Type {
  late var explicitThis = this.toString(); /* expect: avoid-type-to-string */
  late var implicitThis = toString(); /* expect: avoid-type-to-string */
  late var viaSuper = super.toString(); /* expect: avoid-type-to-string */

  void tearOffs() {
    foo(this.toString); /* expect: avoid-type-to-string */
    foo(toString); /* expect: avoid-type-to-string */
  }
}

extension E on Type {
  String describe() => toString(); /* expect: avoid-type-to-string */
}

class Widget {
  String label() => runtimeType.toString(); /* expect: avoid-type-to-string */
}

void typeParameter(Type type) {
  print(type.toString()); /* expect: avoid-type-to-string */
}
