void g1(bool x) {
  while (x) {
    doThing();
  }
}

void g2() {
  for (var i = 0; i < 3; i++) {
    doThing();
  }
}

void g3(bool x) {
  do {
    doThing();
  } while (x);
}

void g4(bool a) {
  if (a) doThing();
}

void g5(bool a, bool b) {
  if (a) {
    doThing();
  } else if (b) {
    doOther();
  } else {
    doThird();
  }
}

void g6(List<int> xs) {
  for (final x in xs) {
    print(x);
  }
}
