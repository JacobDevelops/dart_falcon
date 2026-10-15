/* c */
// @dart = 2.19

void check(Object value) {
  switch (value) {
    case 1 + 2: /* expect: invalid-case-patterns */
      break;
    case true ? 1 : 2: /* expect: invalid-case-patterns */
      break;
    case 1 is int: /* expect: invalid-case-patterns */
      break;
    case 1 is! int: /* expect: invalid-case-patterns */
      break;
    case 'text'.length: /* expect: invalid-case-patterns */
      break;
  }
}

class C {
  static const s = 'text';
}

void staticLength(Object value) {
  switch (value) {
    case C.s.length: /* expect: invalid-case-patterns */
      break;
  }
}
