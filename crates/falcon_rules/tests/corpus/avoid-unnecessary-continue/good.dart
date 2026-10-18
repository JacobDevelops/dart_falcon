void guarded(List<int> values) {
  for (final value in values) {
    if (value < 0) continue;
    print(value);
  }
}

void labeled(List<int> values) {
  outer: for (final first in values) {
    for (final second in values) {
      print(first + second);
      continue outer;
    }
  }
}

void conditionalWork(List<int> values) {
  for (final value in values) {
    if (value < 0) { continue; }
    print(value);
  }
}

void finallyTail(bool ready) {
  while (ready) {
    try { continue; } finally { break; }
  }
}

void laterWork(bool ready) {
  while (ready) {
    { continue; }
    print('later');
  }
}

void noContinue() {
  for (var i = 0; i < 3; i++) { print(i); }
  do { print('once'); } while (false);
}
