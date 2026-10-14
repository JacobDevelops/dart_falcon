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
