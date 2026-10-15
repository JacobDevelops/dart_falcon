// Analyzer-confirmed AdvB ex_const_equality.dart.
class Lists {
  final Object value;
  const Lists._(this.value);
  static const a = Lists._([1]);
  static const b = Lists._(<int>[1]);
  static const c = Lists._(0);
}
void listEquality(Lists value) {
  switch (value) {
    case Lists.a:
    case Lists.c:
      break;
  }
}
class Doubles {
  final double value;
  const Doubles._(this.value);
  static const a = Doubles._(1);
  static const b = Doubles._(1.0);
  static const c = Doubles._(2);
}
void doubleEquality(Doubles value) {
  switch (value) {
    case Doubles.a:
    case Doubles.c:
      break;
  }
}
