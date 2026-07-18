// Wildcards may be declared but never referenced.

void a() {
  var x = 1;
  print(x);
}

int b(int value) => value;

void c(int _) {}

void d() {
  var _ = 1;
}

void e() {
  [1, 2].forEach((_) {});
}

void f() {
  for (var _ in <int>[]) {}
}
