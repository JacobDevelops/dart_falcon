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
