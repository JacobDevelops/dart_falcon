// Good: catch block with content
void processData() {
  try {
    print('doing work');
  } catch (e) {
    print('Error: $e');
  }
}

// Good: method with implementation
class Widget {
  void onInit() {
    print('Initializing');
  }
}

// Good: if block with content
void checkCondition() {
  if (true) {
    print('condition is true');
  }
}

// Good: else block with content
void checkValue(int x) {
  if (x > 0) {
    print('positive');
  } else {
    print('not positive');
  }
}

// Good: empty catch with rethrow or skip comment
void processWithRethrow() {
  try {
    print('work');
  } on Exception {
    rethrow;
  }
}
