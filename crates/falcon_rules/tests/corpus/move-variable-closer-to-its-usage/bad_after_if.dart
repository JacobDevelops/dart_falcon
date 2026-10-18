void afterIf(bool condition) {
  final value = 1; /* expect: move-variable-closer-to-its-usage */
  if (condition) { print('hi'); }
  print(value);
}
void afterSeveral(bool first, bool second) {
  const value = 2; /* expect: move-variable-closer-to-its-usage */
  if (first) { print('first'); }
  if (second) { print('second'); }
  print(value);
  print(value);
}
