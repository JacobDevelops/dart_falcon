void exitsOuterLoop() {
  outer: while (true) {
    try {
      work();
    } finally {
      break outer; /* expect: control-flow-in-finally */
    }
  }
}

extension type Resource(int id) {
  void close() {
    try {
      work();
    } finally {
      return; /* expect: control-flow-in-finally */
    }
  }
}

void work() {}
