class A {
  A(int value);
  factory A.first(int data) => A(data);
  factory A.second(int data) => A(data + 1);
}
class B {
  B(String value);
  factory B.first() => B('a b');
  factory B.second() => B('ab');
}
class C {
  C(int left, int right);
  factory C.first(int x, int y) => C(x, y);
  factory C.second(int a, int b) => C(b, a);
}
class D {
  D(int value);
  factory D.first(int x) => D(x);
}
class E {
  E(int value);
  factory E.first(int x) => E(x);
}
class F {
  F(int value);
  factory F.first(dynamic value) => F(value.first);
  factory F.second(dynamic other) => F(other.second);
}
class H {
  H(int value);
  factory H.first(int data) { final x = data + 1; return H(x); }
  factory H.second(int data) { final x = data + 2; return H(x); }
}
class Foo {
  Foo._();
  int value = 0;
  int count = 0;
  factory Foo.a(int value) => Foo._()..value = value;
  factory Foo.b(int count) => Foo._()..count = count;
}
class V {
  V._(Object value);
  factory V.fromInt(int v) => V._(v);
  factory V.fromString(String v) => V._(v);
}
class P {
  P(double value);
  factory P.small({double v = 4}) => P(v);
  factory P.large({double v = 16}) => P(v);
}
abstract class Q {
  const factory Q.a() = R;
  factory Q.b() = R;
}
class R implements Q { const R(); }
class ConditionalBinding {
  static int value = 7;
  final int result;
  ConditionalBinding._(this.result);
  factory ConditionalBinding.a(bool flag, int value) =>
      ConditionalBinding._(flag ? value : 0);
  factory ConditionalBinding.b(bool flag, int other) =>
      ConditionalBinding._(flag ? value : 0);
}
class NamedLabels {
  NamedLabels._({int value = 0, int other = 0});
  factory NamedLabels.a(int value) => NamedLabels._(value: value);
  factory NamedLabels.b(int other) => NamedLabels._(other: other);
}
class RecordLabels {
  RecordLabels._(Object value);
  factory RecordLabels.a(int value) => RecordLabels._((value: value));
  factory RecordLabels.b(int other) => RecordLabels._((other: other));
}
class RecordTypes {
  RecordTypes._(Object value);
  factory RecordTypes.a(Object value) => RecordTypes._(value as ({int value}));
  factory RecordTypes.b(Object other) => RecordTypes._(other as ({int other}));
}
