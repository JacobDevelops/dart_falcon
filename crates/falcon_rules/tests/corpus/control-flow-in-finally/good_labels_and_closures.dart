void staysInsideFinally() {
  try {
    work();
  } finally {
    inner: while (true) {
      continue inner;
    }
    outer: nested: while (true) {
      if (DateTime.now().millisecondsSinceEpoch.isEven) {
        continue outer;
      }
      continue nested;
    }
    block: {
      break block;
    }
    final delayed = () {
      return;
    };
    delayed();
  }
}

void switchCaseLabelStaysInsideFinally(int value) {
  try {
    work();
  } finally {
    switch (value) {
      case 0:
        continue next;
      next:
      case 1:
        break;
      default:
        break;
    }
  }
}

void work() {}
