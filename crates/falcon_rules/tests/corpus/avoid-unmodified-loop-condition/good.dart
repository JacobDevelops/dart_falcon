void updatedWhile(int count) {
  while (count > 0) {
    count--;
  }
}

void updatedDo(int count) {
  do {
    count -= 1;
  } while (count > 0);
}

void updatedFor() {
  for (var index = 0; index < 10; index++) {
    print(index);
  }
}

void oneInputChanges(int left, int right) {
  while (left < right) {
    right--;
  }
}

void explicitExit(bool running, bool stop) {
  while (running) {
    if (stop) break;
  }
}

void returns(bool running) {
  while (running) {
    return;
  }
}

void throws(bool running) {
  while (running) {
    throw StateError('stopped');
  }
}

void mutatingCondition(int count) {
  while (count-- > 0) {
    print(count);
  }
}

void unknownCondition(bool Function() next) {
  while (next()) {
    print('next');
  }
}

void mutableProperty(List<int> items) {
  while (items.isNotEmpty) {
    items.removeLast();
  }
}

void dynamicCondition(dynamic running) {
  while (running) {
    print(running);
  }
}

bool globalRunning = true;
void globalCondition() {
  while (globalRunning) {
    print(globalRunning);
  }
}

void intentionalInfinite() {
  while (true) {}
  for (;;) {}
}

void closureWrite() {
  var running = true;
  final stop = () { running = false; };
  while (running) {
    stop();
  }
}

void localFunctionWrite() {
  var running = true;
  void stop() { running = false; }
  while (running) {
    stop();
  }
}

Future<void> asynchronousExit(bool running, Future<void> Function() wait) async {
  while (running) {
    await wait();
  }
}

void patternUpdate(int left, int right) {
  while (left < right) {
    (left, right) = (right, left);
  }
}

void nestedWrite(int count, List<int> items) {
  while (count > 0) {
    for (count in items) {
      print(count);
    }
  }
}

void labeledContinue(bool ready) {
  outer: for (var index = 0; index < 10; index++) {
    while (ready) {
      continue outer;
    }
  }
}

Never stop() => throw StateError('stopped');

void neverReturningCall(bool ready) {
  while (ready) {
    stop();
  }
}

class Terminator {
  Never terminate() => throw StateError('stopped');
}

void neverReturningMethod(bool ready, Terminator terminator) {
  while (ready) {
    terminator.terminate();
  }
}
