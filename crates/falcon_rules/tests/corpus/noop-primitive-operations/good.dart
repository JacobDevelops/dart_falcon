class Base {
  @override
  String toString() => 'base';
}

class Child extends Base {
  @override
  String toString() => '${super.toString()}!';
}

void numbers(double d, num n, int? maybe) {
  d.toInt();
  d.round();
  n.toInt();
  n.toDouble();
  (1.5).floor();
}

void strings(String? s, Object o) {
  s?.toString();
  print('value: $o');
  print(o);
  print(o.toString().length);
  print('$s'.length);
  final edges = '' 'a' 'b' '';
  print(edges);
  debugPrint(o.toString());
}

void debugPrint(String message) {}

// Mixed `int` and `double` operands make `clamp` return a `num`.
void mixedClamp(double d, int? maybe) {
  d.clamp(0, 1).toDouble();
  (maybe ?? 1.5).toDouble();
}
