// Each function nests control structures deeper than the configured
// max_nesting of 2 (deepest point reaches nesting level 3).

void ifForIf(bool a, bool b, List<int> xs) { /* expect: maximum-nesting-level */
  if (a) {
    for (final x in xs) {
      if (b) {
        print(x);
      }
    }
  }
}

void whileIfWhile(int n) { /* expect: maximum-nesting-level */
  while (n > 0) {
    if (n.isEven) {
      while (n > 10) {
        n -= 1;
      }
    }
    n -= 1;
  }
}

void switchInLoop(List<int> codes) { /* expect: maximum-nesting-level */
  for (final code in codes) {
    if (code > 0) {
      switch (code) {
        case 1:
          print('one');
        default:
          print('other');
      }
    }
  }
}

void tryInside(bool a, bool b) { /* expect: maximum-nesting-level */
  if (a) {
    if (b) {
      try {
        print('run');
      } catch (e) {
        print(e);
      }
    }
  }
}

void deepLoops(List<List<int>> grid) { /* expect: maximum-nesting-level */
  for (final row in grid) {
    for (final cell in row) {
      if (cell > 0) {
        print(cell);
      }
    }
  }
}

class Grid {
  void render(List<List<int>> rows) { /* expect: maximum-nesting-level */
    for (final row in rows) {
      for (final cell in row) {
        while (cell > 0) {
          print(cell);
          break;
        }
      }
    }
  }
}
