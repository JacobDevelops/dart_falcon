void elseIfChain(bool a, bool b, bool c) {
  if (a) {} else if (b) {} else if (c) {}
}

void functionBoundaries(bool a, bool b) {
  if (a) {
    while (b) {
      final callbacks = [() {
        if (a) {
          while (b) { break; }
        }
      }];
      void local() {
        if (a) {
          while (b) { break; }
        }
      }
      break;
    }
  }
}
