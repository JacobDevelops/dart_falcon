void target({int alpha = 0, int zebra = 0}) {}
class Example {
  const Example({int alpha = 0, int zebra = 0});
  Example.forward() : this(zebra: 2, /* expect: arguments-ordering */ alpha: 1);
}
class Child extends Example {
  Child() : super(zebra: 2, /* expect: arguments-ordering */ alpha: 1);
}
void calls() {
  target(zebra: 2, /* expect: arguments-ordering */ alpha: 1);
  Example(zebra: 2, /* expect: arguments-ordering */ alpha: 1);
  new Example(zebra: 2, /* expect: arguments-ordering */ alpha: 1);
  const Example(zebra: 2, /* expect: arguments-ordering */ alpha: 1);
}

@Example(zebra: 2, /* expect: arguments-ordering */ alpha: 1)
class Annotated {}

enum Choice {
  value(zebra: 2, /* expect: arguments-ordering */ alpha: 1);
  const Choice({int alpha = 0, int zebra = 0});
}

class Builder {
  void configure({int alpha = 0, int zebra = 0}) {}
}
void cascades(Builder builder) {
  builder..configure(zebra: 2, /* expect: arguments-ordering */ alpha: 1);
}

void declarationOrder({int zebra = 0, int alpha = 0, int middle = 0}) {}
class NonAlphabetical {
  const NonAlphabetical({int zebra = 0, int alpha = 0});
  const NonAlphabetical.named({int zebra = 0, int alpha = 0});
  NonAlphabetical.forward()
      : this.named(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
  void configure({int zebra = 0, int alpha = 0}) {}
}
void declarationCalls(NonAlphabetical object) {
  declarationOrder(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
  declarationOrder(middle: 3, /* expect: arguments-ordering */ alpha: 1);
  NonAlphabetical(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
  new NonAlphabetical(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
  object.configure(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
  object..configure(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
}

@NonAlphabetical(alpha: 1, /* expect: arguments-ordering */ zebra: 2)
class DeclarationAnnotation {}

class Inherited extends NonAlphabetical {
  Inherited() : super(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
  Inherited.named()
      : super.named(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
}
void inheritedCall(Inherited object) {
  object.configure(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
}

@NonAlphabetical.named(alpha: 1, /* expect: arguments-ordering */ zebra: 2)
class NamedDeclarationAnnotation {}

enum NamedChoice {
  value.named(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
  const NamedChoice.named({int zebra = 0, int alpha = 0});
}

class Generic<T> {
  void configure({T? zebra, T? alpha}) {}
}
void genericCall(Generic<int> object) {
  object.configure(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
}

class PrivatePrimary({final int _alpha = 0, final int _zebra = 0}) {}
class MixedPrimary({final int _zebra = 0, int alpha = 0, var _middle = 0}) {}
void primaryCalls() {
  PrivatePrimary(zebra: 2, /* expect: arguments-ordering */ alpha: 1);
  MixedPrimary(alpha: 1, /* expect: arguments-ordering */ zebra: 2, middle: 3);
}
