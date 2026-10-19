void f(int x, int y, bool a, bool b) {
  if (x != y) {}
  if (x == y) {}
  if (!a) {}
  final c = !!a;
  final d = !a || !b;
  final e = !(x + y).isEven;
  final inverted = !(a || b);
  final comparison = !(x == y);
  final callback = () => !(x > y);
}

void relational(double x, int y, GreaterOnly custom) {
  if (!(x > 0)) {}
  if (!(x >= 0)) {}
  if (!(x < 0)) {}
  if (!(x <= 0)) {}
  final d = !((x <= y)) ? 1 : 2;
  if (!(custom > 0)) {}
}

class GreaterOnly {
  bool operator >(int other) => true;
}
