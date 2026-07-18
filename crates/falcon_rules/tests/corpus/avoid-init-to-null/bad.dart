int? topLevel = null; /* expect: avoid-init-to-null */
var inferred = null; /* expect: avoid-init-to-null */

class C {
  String? field = null; /* expect: avoid-init-to-null */
  dynamic dyn = null; /* expect: avoid-init-to-null */

  // Regression: a `= null` nested inside a closure in a field initializer.
  final closureField = () {
    var inner = null; /* expect: avoid-init-to-null */
    return inner;
  };

  void method() {
    int? local = null; /* expect: avoid-init-to-null */
    var x = null; /* expect: avoid-init-to-null */
    double? a = null, b; /* expect: avoid-init-to-null */
    print('$local $x $a $b');
  }
}
