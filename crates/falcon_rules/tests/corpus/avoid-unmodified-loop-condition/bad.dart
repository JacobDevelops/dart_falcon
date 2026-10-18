void unchangedBool(bool running) {
  while (running) { /* expect: avoid-unmodified-loop-condition */
    print(running);
  }
}

void unchangedComparison(int count, int limit) {
  while (count < limit) { /* expect: avoid-unmodified-loop-condition */
    print(count);
  }
  count++;
}

void unchangedString(String text) {
  while (text.isNotEmpty) { /* expect: avoid-unmodified-loop-condition */
    print(text);
  }
}

void unchangedDo(bool running) {
  do {
    print(running);
  } while (running); /* expect: avoid-unmodified-loop-condition */
}

void unrelatedUpdate(int count) {
  for (var index = 0; count > 0; index++) { /* expect: avoid-unmodified-loop-condition */
    print(index);
  }
}

void shadowedWrite(int count) {
  while (count > 0) { /* expect: avoid-unmodified-loop-condition */
    var count = 1;
    count--;
    print(count);
  }
}

void initializationIsNotUpdate() {
  for (var index = 0; index < 10;) { /* expect: avoid-unmodified-loop-condition */
    print(index);
  }
}

void multipleInputs(int left, int right, bool active) {
  while (active && left != right) { /* expect: avoid-unmodified-loop-condition */
    print(left);
  }
}

void inferredLiteral() {
  var running = true;
  while (!(!running)) { /* expect: avoid-unmodified-loop-condition */
    print(running);
  }
}
