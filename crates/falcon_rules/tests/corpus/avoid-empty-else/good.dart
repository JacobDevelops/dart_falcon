void g1(bool a) {
  if (a) {
    print('a');
  } else {
    print('b');
  }
}

void g2(int n) {
  if (n > 0) {
    print('pos');
  }
}

void g3(bool a, bool b) {
  if (a) {
    print('a');
  } else if (b) {
    print('b');
  } else {
    print('c');
  }
}

void g4(bool a) {
  if (a) print('y');
}

void g5(bool a) {
  if (a) {
    print('a');
  }
}

void g6(bool a) {
  while (a) {
    if (a) {
      print('x');
    } else {
      print('y');
    }
  }
}
