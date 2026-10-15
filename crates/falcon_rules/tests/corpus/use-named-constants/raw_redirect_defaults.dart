abstract class Sequence<T extends Object> {
  static const number = Single<bool, int>(true, 1);
  const factory Sequence.single(T value, int other) = Single;
}

class Single<T extends Object, U> implements Sequence<T> {
  final T value;
  final U other;
  const Single(this.value, this.other);
}

// U is unconstrained by the factory return contract and defaults to dynamic;
// its runtime argument value must not change the redirect to Single<bool, int>.
void use() {
  const Sequence.single(true, 1);
}

abstract class DynamicSequence<T extends Object> {
  static const number = DynamicSingle<bool, dynamic>(true, 1);
  const factory DynamicSequence.single(T value, int other) = DynamicSingle;
}

class DynamicSingle<T extends Object, U> implements DynamicSequence<T> {
  final T value;
  final U other;
  const DynamicSingle(this.value, this.other);
}

void useDefault() {
  const DynamicSequence.single(true, 1); /* expect: use-named-constants */
}
