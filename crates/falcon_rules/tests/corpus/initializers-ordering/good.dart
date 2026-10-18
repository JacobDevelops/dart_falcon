int next() => 1;

class Example {
  final int zebra;
  final int alpha;
  Example() : zebra = 2, alpha = 1;
  Example.values(int a, int z) : zebra = z, alpha = a;
  Example.withAssert() : alpha = 1, assert(true), zebra = 2;
  Example.withEffects() : zebra = next(), alpha = next();
  Example.withUnknownValue() : zebra = next(), alpha = 1;
  Example.reversedEffects() : alpha = next(), zebra = next();
  Example.reversedUnknownValue() : alpha = next(), zebra = 1;
  Example.redirect() : this();
}

class Parent {
  Parent();
}

class Child extends Parent {
  final int zebra;
  final int alpha;
  Child() : zebra = 2, alpha = 1, super();
  Child.withAssert() : alpha = 1, assert(true), zebra = 2, super();
}

class Words {
  final String zebra, alpha;
  Words() : zebra = 'z', alpha = 'a';
  Words.withInterpolation() : zebra = '${next()}', alpha = 'a';
  Words.reversedInterpolation() : alpha = '${next()}', zebra = 'z';
}

// Each containing class has its own declaration order.
class Opposite {
  final int alpha;
  final int zebra;
  Opposite() : alpha = 1, zebra = 2;
}

enum EnumFields {
  value;

  final int zebra;
  final int alpha;
  const EnumFields() : zebra = 2, alpha = 1;
}
