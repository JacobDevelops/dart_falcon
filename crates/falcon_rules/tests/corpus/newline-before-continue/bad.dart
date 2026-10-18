void loops(bool condition) {
  while (condition) {
    print('before');
    continue; /* expect: newline-before-continue */
  }
  outer: for (var i = 0; i < 3; i++) {
    while (condition) {
      print('nested');
      continue outer; /* expect: newline-before-continue */
    }
  }
}

void commented(bool condition) {
  while (condition) {
    print('before');
    // No separating blank line.
    continue; /* expect: newline-before-continue */
  }
}
