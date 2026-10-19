void handle(int x) {
  switch (x) { case 0:
    switch (x + 1) { /* expect: avoid-nested-switches */
      case 1: break;
    }
  }
}
