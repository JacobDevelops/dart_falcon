void g1() {
  try {
    doThing();
  } finally {
    for (var i = 0; i < 10; i++) {
      if (i > 5) break;
    }
  }
}

void g2(int x) {
  try {
    doThing();
  } finally {
    switch (x) {
      case 1:
        break;
      default:
        break;
    }
  }
}

void g3() {
  try {
    doThing();
  } finally {
    final f = () {
      return;
    };
    f();
  }
}

void g4() {
  try {
    doThing();
  } finally {
    doCleanup();
  }
}

int g5() {
  try {
    return 1;
  } finally {
    doCleanup();
  }
}

void g6(bool cond) {
  try {
    doThing();
  } finally {
    while (cond) {
      continue;
    }
  }
}
