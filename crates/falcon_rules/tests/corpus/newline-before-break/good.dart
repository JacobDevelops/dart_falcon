void loops(bool condition) {
  while (condition) {
    break;
  }
  while (condition) {
    print('before');

    break;
  }
  outer: for (var i = 0; i < 3; i++) {
    while (condition) {
      break outer;
    }
  }
  while (condition) {
    if (condition) break;
  }
}

void cases(int value) {
  switch (value) {
    default:
      print(value);
      break;
  }
}

void commented(bool condition) {
  while (condition) {
    print('before');

    // Stop this loop.
    break;
  }
}
