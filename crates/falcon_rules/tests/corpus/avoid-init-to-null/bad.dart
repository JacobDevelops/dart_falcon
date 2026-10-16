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

void namedDefault({int? value = null}) {} /* expect: avoid-init-to-null */
void positionalDefault([int? value = null]) {} /* expect: avoid-init-to-null */

class FieldDefault {
  final int? value;
  FieldDefault({this.value = null}); /* expect: avoid-init-to-null */
}

class NullSuperDefault {
  NullSuperDefault({int? value});
}

class RedundantSuperDefault extends NullSuperDefault {
  RedundantSuperDefault({super.value = null}); /* expect: avoid-init-to-null */
}

void parenthesizedDefault({int? value = (null)}) {} /* expect: avoid-init-to-null */

class RequiredPositionalBase {
  RequiredPositionalBase(int? value);
}

class OptionalPositionalChild extends RequiredPositionalBase {
  OptionalPositionalChild([super.value = null]); /* expect: avoid-init-to-null */
}

class RequiredNamedBase {
  RequiredNamedBase({required int? value});
}

class OptionalNamedChild extends RequiredNamedBase {
  OptionalNamedChild({super.value = null}); /* expect: avoid-init-to-null */
}
