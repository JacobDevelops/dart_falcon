void select(int value) {
  switch (value) {
    case 0:
      print(0);
      break;
    case 1: /* expect: newline-before-case */
      switch (value) {
        case 1:
          break;
        default: /* expect: newline-before-case */
          break;
      }
      break;
    default: /* expect: newline-before-case */
      break;
  }
}

void commented(int value) {
  switch (value) {
    case 0:
      break;
    // No separating blank line.
    default: /* expect: newline-before-case */
      break;
  }
}
