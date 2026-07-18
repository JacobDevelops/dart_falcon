void f1(bool a) {
  if (a) {
    print('a');
  } else ; /* expect: avoid-empty-else */
}

void f2(int n) {
  if (n > 0) print('pos'); else ; /* expect: avoid-empty-else */
}

void f3(bool a) {
  if (a) {
    print('a');
  } else
    ; /* expect: avoid-empty-else */
}

void f4(bool a, bool b) {
  if (a) {
    print('a');
  } else if (b) {
    print('b');
  } else ; /* expect: avoid-empty-else */
}

void f5(bool a) {
  while (a) {
    if (a) {
      print('x');
    } else ; /* expect: avoid-empty-else */
  }
}

void f6(bool a) {
  if (a) print('y'); else ; /* expect: avoid-empty-else */
}
