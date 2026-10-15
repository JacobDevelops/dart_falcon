// @dart = 2.19
// Analyzer-confirmed AdvB3 expression cases; parsing is version-sensitive.

void length(Object o) {
  switch (o) {
    case 'a'.length: /* expect: invalid-case-patterns */
      break;
  }
}
