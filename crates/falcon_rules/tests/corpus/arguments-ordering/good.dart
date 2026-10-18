void target(int zebra, int alpha, {int beta = 0, int gamma = 0}) {}
class Example {
  const Example({int alpha = 0, int zebra = 0});
}
void calls() {
  target(2, 1, beta: 3, gamma: 4);
  Example(alpha: 1, zebra: 2);
  const Example(alpha: 1, zebra: 2);
  final items = [2, 1];
  final record = (zebra: 2, alpha: 1);
}

void declarationOrder({int zebra = 0, int alpha = 0, int middle = 0}) {}
class NonAlphabetical {
  const NonAlphabetical({int zebra = 0, int alpha = 0});
  const NonAlphabetical.named({int zebra = 0, int alpha = 0});
  NonAlphabetical.forward() : this.named(zebra: 2, alpha: 1);
  void configure({int zebra = 0, int alpha = 0}) {}
}
void declarationCalls(NonAlphabetical object, dynamic unknown) {
  declarationOrder(zebra: 2, alpha: 1, middle: 3);
  declarationOrder(zebra: 2, middle: 3);
  NonAlphabetical(zebra: 2, alpha: 1);
  new NonAlphabetical(zebra: 2, alpha: 1);
  object.configure(zebra: 2, alpha: 1);
  object..configure(zebra: 2, alpha: 1);
  unknown.configure(zebra: 2, alpha: 1);
  unknown.configure(alpha: 1, zebra: 2);
  unresolved(zebra: 2, alpha: 1);
  unresolved(alpha: 1, zebra: 2);
}

@NonAlphabetical(zebra: 2, alpha: 1)
class DeclarationAnnotation {}

class Inherited extends NonAlphabetical {
  Inherited() : super(zebra: 2, alpha: 1);
  Inherited.named() : super.named(zebra: 2, alpha: 1);
  Inherited.forward() : this();
}
void inheritedCall(Inherited object) {
  object.configure(zebra: 2, alpha: 1);
}
void functionTypeUnknown(void Function({int alpha, int zebra}) callback) {
  callback(zebra: 2, alpha: 1);
}

@NonAlphabetical.named(zebra: 2, alpha: 1)
class NamedDeclarationAnnotation {}

enum NamedChoice {
  value.named(zebra: 2, alpha: 1);
  const NamedChoice.named({int zebra = 0, int alpha = 0});
}

class Generic<T> {
  void configure({T? zebra, T? alpha}) {}
}
void genericCall(Generic<int> object) {
  object.configure(zebra: 2, alpha: 1);
}

class PrivatePrimary({final int _alpha = 0, final int _zebra = 0}) {}
class MixedPrimary({final int _zebra = 0, int alpha = 0, var _middle = 0}) {}
void primaryCalls() {
  PrivatePrimary(alpha: 1, zebra: 2);
  MixedPrimary(zebra: 2, alpha: 1, middle: 3);
  MixedPrimary(zebra: 2, middle: 3);
}
