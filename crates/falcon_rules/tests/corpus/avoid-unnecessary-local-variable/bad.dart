void a() {
  var first = 1; /* expect: avoid-unnecessary-local-variable */
  var second = first;
  print(second);
}
void b() {
  final first = Object(); /* expect: avoid-unnecessary-local-variable */
  final second = first;
  print(second);
}
void c() {
  final first = [1, 2]; /* expect: avoid-unnecessary-local-variable */
  final second = first;
  print(second);
}
void d() {
  final first = true; /* expect: avoid-unnecessary-local-variable */
  final second = (first);
  print(second);
}
void e() {
  final first = DateTime.now(); /* expect: avoid-unnecessary-local-variable */
  var second = first;
  print(second);
}
