// Redirected factories retain their owner substitutions through a generic
// superclass. Raw redirects infer the target from the factory's return type.
abstract class Sequence<T extends Object> {
  static const yes = Single<bool>(true);
  static const no = Single<bool>(false);
  static const empty = Single<String>('');
  static const nan = Single<double>(0.0 / 0.0);
  static const nested = Single<Single<Single<bool>>>(
    Single<Single<bool>>(Single<bool>(true)),
  );
  static const list = Single<List<int>>(<int>[1]);
  const factory Sequence.single(T value) = Single<T>;
}

class Base<T> {
  const Base();
}

class Single<T extends Object> extends Base<T> implements Sequence<T> {
  final T value;
  const Single(this.value);
}

void use() {
  const Sequence.single(true); /* expect: use-named-constants */
  const Sequence.single(false); /* expect: use-named-constants */
  const Sequence.single(''); /* expect: use-named-constants */
  const Sequence.single(0.0 / 0.0); /* expect: use-named-constants */
  const Sequence.single(Single(Single(true))); /* expect: use-named-constants */
  const Sequence.single([1]); /* expect: use-named-constants */
  const Sequence.single(1);
  const Object wide = 0.0 / 0.0;
  const Sequence.single(wide);
  const Sequence<Object>.single(0.0 / 0.0);
}

abstract class Raw<T extends Object> {
  static const yes = RawSingle<bool>(true);
  const factory Raw.single(T value) = RawSingle;
}

class RawSingle<T extends Object> extends Base<T> implements Raw<T> {
  final T value;
  const RawSingle(this.value);
}

void raw() {
  const Raw.single(true); /* expect: use-named-constants */
}

// A raw result/argument/collection context instantiates Sequence<Object>.
// Its Single<Object>(true) is not the canonical Single<bool>(true).
Sequence broadResult() => const Sequence.single(true);
Sequence<bool> preciseResult() =>
    const Sequence.single(true); /* expect: use-named-constants */

void takeSequence(Sequence value) {}

void downwardContexts() {
  takeSequence(const Sequence.single(true));
  final List<Sequence> values = [const Sequence.single(true)];
  const Sequence.single(true); /* expect: use-named-constants */
  const Sequence<Object>.single(true);
}
