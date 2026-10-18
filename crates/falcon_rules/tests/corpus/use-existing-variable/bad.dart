int a() {
  final x = 2;
  final cached = x + 1;
  return x + 1; /* expect: use-existing-variable */
}
int b() {
  final x = 3;
  final cached = x * 2;
  return (x * 2); /* expect: use-existing-variable */
}
int c() {
  final x = 4;
  final cached = x - 1;
  final next = x - 1; /* expect: use-existing-variable */
  return next;
}
int d() {
  final x = 5;
  final cached = (x + 1) * 2;
  return (x + 1) * 2; /* expect: use-existing-variable */
}
int e() {
  final cached = 2 + 3;
  return 2 + 3; /* expect: use-existing-variable */
}
void sdkGetter(String value) {
  final cached = value.length.isOdd;
  print(value.length.isOdd); /* expect: use-existing-variable */
}
