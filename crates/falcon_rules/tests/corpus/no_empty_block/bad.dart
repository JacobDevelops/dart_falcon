// Bad: empty catch block
void processData() {
  try {
    print('doing work');
  } catch (e) { /* expect: no_empty_block */
  }
}

// Bad: empty method body
class Widget {
  void onInit() { /* expect: no_empty_block */
  }
}

// Bad: empty if block
void checkCondition() {
  if (true) { /* expect: no_empty_block */
  }
}

// Bad: empty else block
void checkValue(int x) {
  if (x > 0) {
    print('positive');
  } else { /* expect: no_empty_block */
  }
}

// Bad: empty for loop
void loopWithoutBody() {
  for (int i = 0; i < 10; i++) { /* expect: no_empty_block */
  }
}

// Bad: empty while loop
void whileWithoutBody() {
  int count = 0;
  while (count < 5) { /* expect: no_empty_block */
  }
}

// Bad: empty finally block
void tryWithEmptyFinally() {
  try {
    print('something');
  } finally { /* expect: no_empty_block */
  }
}
