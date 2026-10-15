void foo(String Function() f) {}

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

extension E on Type2 {
  String describe() => toString(); /* expect: avoid-type-to-string */
}
