void a(int next) {
  int x = 1;
  x = next; /* expect: avoid-unnecessary-reassignment */
  print(x);
}
void b(bool next) {
  bool x = false;
  x = next; /* expect: avoid-unnecessary-reassignment */
  print(x);
}
void c(double next) {
  double x = 1.5;
  x = next; /* expect: avoid-unnecessary-reassignment */
  print(x);
}
void d(Object next) {
  Object? x = null;
  x = next; /* expect: avoid-unnecessary-reassignment */
  print(x);
}
void e(bool flag) {
  int x = 0;
  x = flag ? 1 : 2; /* expect: avoid-unnecessary-reassignment */
  print(x);
}
void stringInitializer(String next) {
  String alpha = '';
  alpha = next; /* expect: avoid-unnecessary-reassignment */
  print(alpha);
}
