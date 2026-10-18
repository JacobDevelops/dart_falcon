void loops(bool condition) {
  while (condition) {
    print('before');
    break; /* expect: newline-before-break */
  }
  outer: for (var i = 0; i < 3; i++) {
    while (condition) {
      print('nested');
      break outer; /* expect: newline-before-break */
    }
  }
}

void commented(bool condition) {
  while (condition) {
    print('before');
    // No separating blank line.
    break; /* expect: newline-before-break */
  }
}
