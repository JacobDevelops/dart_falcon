class Point {
  final int x;
  Point(this.x);
}

T make<T>() => throw UnimplementedError();

void f(int n, String s, List<int> items, dynamic dyn) {
  var a = 1;
  final b = 'text';
  num c = 1;
  double d = 1;
  int? e = n;
  Object f = s;
  int g = make();
  Iterable<int> h = [];
  List<int>? i = [];
  Set<int>? q = {};
  dynamic j = n;
  int k = dyn;
  int l = n, m = dyn;
  int o;
  o = 1;
  for (num item in items) {}
  for (var item in items) {}
  print([a, b, c, d, e, f, g, h, i, j, k, l, m, o, q]);
}

T identity<T>(T value) => value;
void genericReturnPolicy() {
  int argument = identity(1);
  T localIdentity<T>(T value) => value;
  int localArgument = localIdentity(1);
  print([argument, localArgument]);
}
