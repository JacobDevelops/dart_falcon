class A {
  A(int value);
  factory A.first(int data) => A(data);
  factory A.second(int renamed) => A(renamed); /* expect: avoid-duplicate-factories */
}
class B {
  B();
  factory B.empty() => B();
  factory B.withData(String unused) => B(); /* expect: avoid-duplicate-factories */
}
class C {
  C(int value);
  factory C.first(int x) { return C(x + 1); }
  factory C.second(int y) => C(y + 1); /* expect: avoid-duplicate-factories */
}
class D {
  D(int left, int right);
  factory D.first(int x, int y) => D(x, y);
  factory D.second(int a, int b) => D(a, b); /* expect: avoid-duplicate-factories */
}
class E {
  E(String value);
  factory E.first() => E('constant');
  factory E.second() => E( 'constant' ); /* expect: avoid-duplicate-factories */
}
class F {
  factory F.first() = G;
  factory F.second() = G; /* expect: avoid-duplicate-factories */
}
class G implements F { G(); }
class ConditionalBinding {
  final int result;
  ConditionalBinding._(this.result);
  factory ConditionalBinding.a(bool flag, int value) =>
      ConditionalBinding._(flag ? value : 0);
  factory ConditionalBinding.b(bool flag, int other) => ConditionalBinding._(flag ? other : 0); /* expect: avoid-duplicate-factories */
}
class MapKeys {
  MapKeys._(Map<int, int> values);
  factory MapKeys.a(int value) => MapKeys._({value: 0});
  factory MapKeys.b(int other) => MapKeys._({other: 0}); /* expect: avoid-duplicate-factories */
}
class RecordValues {
  RecordValues._(Object value);
  factory RecordValues.a(int value) => RecordValues._((result: value));
  factory RecordValues.b(int other) => RecordValues._((result: other)); /* expect: avoid-duplicate-factories */
}
class RecordTypes {
  RecordTypes._(Object value);
  factory RecordTypes.a(Object value) => RecordTypes._(value as ({int value}));
  factory RecordTypes.b(Object other) => RecordTypes._(other as ({int value})); /* expect: avoid-duplicate-factories */
}
