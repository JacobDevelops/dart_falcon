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

void nestedFinallyReportsOnce() {
  try {
    work();
  } finally {
    try {
      work();
    } finally {
      return; /* expect: control-flow-in-finally */
    }
  }
}

void innerFinallyCannotBreakOuterFinallyLoop() {
  try {
    work();
  } finally {
    cleanup: while (true) {
      try {
        work();
      } finally {
        break cleanup; /* expect: control-flow-in-finally */
      }
    }
  }
}

void nestedFinallyInsideFunctionBoundaries() {
  try {
    work();
  } finally {
    final callback = () {
      try {
        work();
      } finally {
        return; /* expect: control-flow-in-finally */
      }
    };
    void local() {
      try {
        work();
      } finally {
        return; /* expect: control-flow-in-finally */
      }
    }
    callback();
    local();
  }
}
