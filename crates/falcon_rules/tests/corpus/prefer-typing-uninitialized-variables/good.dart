// Either a type annotation or an initializer is present.
int topLevel = 0;

void f() {
  int a;
  var b = 1;
  final c = 2;
  String? d;
  double e = 0.0;
  a = 5;
  d = 'x';
  print(b + c + e + a + d.length);
}

class C {
  int field = 0;
  var typed = 'x';
  static int s = 1;
}
