void example(bool enabled) {
  final count = 3; /* expect: move-variable-closer-to-its-usage */
  if (enabled) {
    print(count);
    print(count);
  }
}
void alternative(bool enabled) {
  final value = 1.5; /* expect: move-variable-closer-to-its-usage */
  if (enabled) {
    print('enabled');
  } else {
    print(value);
  }
}
void shadowed(bool enabled) {
  final count = 3; /* expect: move-variable-closer-to-its-usage */
  if (enabled) { print(count); }
  { final count = 4; print(count); }
}
void constant(bool enabled) {
  const count = 3; /* expect: move-variable-closer-to-its-usage */
  if (enabled) { print(count); }
}
void boolean(bool enabled) {
  final value = false; /* expect: move-variable-closer-to-its-usage */
  if (enabled) { print(value); }
}
