int? topLevel = null; /* expect: avoid-init-to-null */
var inferred = null; /* expect: avoid-init-to-null */

class C {
  String? field = null; /* expect: avoid-init-to-null */
  dynamic dyn = null; /* expect: avoid-init-to-null */

  void method() {
    int? local = null; /* expect: avoid-init-to-null */
    var x = null; /* expect: avoid-init-to-null */
    double? a = null, b; /* expect: avoid-init-to-null */
    print('$local $x $a $b');
  }
}
