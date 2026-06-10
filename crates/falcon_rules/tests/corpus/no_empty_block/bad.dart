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
