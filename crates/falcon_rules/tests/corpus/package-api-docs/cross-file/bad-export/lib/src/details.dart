class Exposed { /* expect: package-api-docs */
  Exposed(); /* expect: package-api-docs */
  int field = 0; /* expect: package-api-docs */
  void method() {} /* expect: package-api-docs */
  int get value => 0; /* expect: package-api-docs */
  set value(int v) {} /* expect: package-api-docs */
  @override
  String toString() => '';
}
class Hidden { void method() {} }
class NotShown {}
