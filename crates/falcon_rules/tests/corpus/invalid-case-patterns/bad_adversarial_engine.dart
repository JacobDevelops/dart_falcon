// @dart = 2.19
const x = 1;
void compare(Object o) {
  switch (o) {
    case 1 == 2: /* expect: invalid-case-patterns */
      break;
  }
}
void negate(Object o) {
  switch (o) {
    case -x: /* expect: invalid-case-patterns */
      break;
  }
}
