// Each function keeps control-structure nesting at or below the
// max_nesting of 2, so none trigger the rule.

void singleIf(bool a) {
  if (a) {
    print('yes');
  }
}

void loopThenIf(List<int> xs) {
  for (final x in xs) {
    print(x);
  }
  if (xs.isEmpty) {
    print('empty');
  }
}

void ifInLoop(List<int> xs, bool flag) {
  for (final x in xs) {
    if (flag) {
      print(x);
    }
  }
}

void guardedWhile(int n) {
  while (n > 0) {
    n -= 1;
  }
}

void tryOnce(bool a) {
  if (a) {
    try {
      print('run');
    } catch (e) {
      print(e);
    }
  }
}

class Printer {
  void printAll(List<int> xs) {
    for (final x in xs) {
      print(x);
    }
  }
}

// Labeled loop whose nesting stays within the limit.
void labeledShallow(List<int> xs, bool a) {
  loop:
  for (final x in xs) {
    if (a) {
      print(x);
      break loop;
    }
  }
}

// Closure whose nesting stays within the limit.
void closureShallow(List<int> xs) {
  run(() {
    for (final x in xs) {
      print(x);
    }
  });
}
