// @dart = 2.19
// Legacy switches retain expression-case duplicate checks.
const original = 3;
const alias = original;

void legacy(int value) {
  switch (value) {
    case 1 + 2:
      break;
    case alias: /* expect: no-duplicate-case-values */
      break;
  }
}
