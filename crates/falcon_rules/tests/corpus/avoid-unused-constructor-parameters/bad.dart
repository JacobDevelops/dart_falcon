// @dart = 3.13

class BadOne {
  BadOne(int unused, [String? empty]); /* expect: avoid-unused-constructor-parameters */ /* expect: avoid-unused-constructor-parameters */
}

class BadTwo {
  int c;

  BadTwo(
    int a, /* expect: avoid-unused-constructor-parameters */
    this.c,
  );
}

class BadThree {
  final int value;

  BadThree(int input, {int? spare}) : value = input; /* expect: avoid-unused-constructor-parameters */
}

class BadFour {
  int total = 0;

  BadFour(int first, int second) { /* expect: avoid-unused-constructor-parameters */
    total = first;
  }
}

class Base {
  Base(this.x);
  final int x;
}

class BadFive extends Base {
  BadFive(super.x, int extra); /* expect: avoid-unused-constructor-parameters */
}

class BadSix {
  final int value;

  BadSix._(this.value);

  factory BadSix.make(int value, String label) { /* expect: avoid-unused-constructor-parameters */
    return BadSix._(value);
  }
}

enum BadEnum {
  a(1);

  const BadEnum(int unused); /* expect: avoid-unused-constructor-parameters */
}

class BadClosureShadow {
  BadClosureShadow(int x) { /* expect: avoid-unused-constructor-parameters */
    void inner(int x) {
      print(x);
    }

    inner(1);
  }
}

// Primary cases follow the SDK 3.13.5 rule's parameter/initializer scopes.
class PrimaryUnused(int unused); /* expect: avoid-unused-constructor-parameters */

class PrimaryOptionalUnused([int unused = 0]); /* expect: avoid-unused-constructor-parameters */

class PrimaryNamedUnused.named({int unused = 0}); /* expect: avoid-unused-constructor-parameters */

class PrimaryPrivateUnused._(int unused) { /* expect: avoid-unused-constructor-parameters */
  this;
}

class PrimaryPartiallyUsed(int used, int unused) { /* expect: avoid-unused-constructor-parameters */
  final int value = used;
}

class PrimaryBodyShadow(int value) { /* expect: avoid-unused-constructor-parameters */
  this {
    void inner(int value) {
      print(value);
    }
    inner(1);
  }
}

class PrimaryFieldShadow(int value) { /* expect: avoid-unused-constructor-parameters */
  final int Function(int) callback = (int value) => value;
}

class PrimaryMethodScope(int value) { /* expect: avoid-unused-constructor-parameters */
  int method(int value) => value;
}

class PrimarySiblingScope(int value) { /* expect: avoid-unused-constructor-parameters */
  PrimarySiblingScope.other(int value) : this(value);
}

int outerValue = 1;

class PrimaryLateScope(int outerValue) { /* expect: avoid-unused-constructor-parameters */
  late final int value = outerValue;
}

class PrimaryStaticScope(int outerValue) { /* expect: avoid-unused-constructor-parameters */
  static final int value = outerValue;
}

// INVALID-CODE: non-trivial mixin constructor; analyzer still reports the unused parameter.
mixin class PrimaryMixinUnused(int unused) {} /* expect: avoid-unused-constructor-parameters */

enum PrimaryEnumUnused(int unused) { /* expect: avoid-unused-constructor-parameters */
  a(1)
}
