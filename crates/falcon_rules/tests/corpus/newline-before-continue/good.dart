void loops(bool condition) {
  while (condition) {
    continue;
  }
  while (condition) {
    print('before');

    continue;
  }
  outer: for (var i = 0; i < 3; i++) {
    while (condition) {
      continue outer;
    }
  }
  while (condition) {
    if (condition) continue;
  }
}

void cases(int value) {
  while (true) {
    switch (value) {
      case 0:
        print(value);
        continue next;
      next:
      default:
        return;
    }
  }
}

void commented(bool condition) {
  while (condition) {
    print('before');

    // Start another iteration.
    continue;
  }
}
