// A generic mixin class raw in an annotation and in a constructor invocation
// whose type argument would be inferred.
abstract mixin class Seq<T extends Object> {
  const factory Seq.single(T value) = _Single<T>;
}

class _Single<T extends Object> with Seq<T> {
  const _Single(this.value);
  final T value;
}

Seq<int> one(
  Seq<int> Function(Seq<int>, Seq raw) f, /* expect: always-specify-types */
) =>
    const Seq.single(1); /* expect: always-specify-types */

List<int> loop(List<int> xs) => <int>[
  for (final int x in xs) x,
  for (final x in xs) x, /* expect: always-specify-types */
];

void callback(void cb(), handler()) {} /* expect: always-specify-types */
