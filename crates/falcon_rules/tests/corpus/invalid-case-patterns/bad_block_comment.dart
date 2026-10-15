/* c */
// @dart = 2.19
void f(Object o) {
  switch (o) {
    case [1]: /* expect: invalid-case-patterns */
      break;
  }
}
