// Test cases for format-comment rule
// No violations: all comments start with uppercase or are empty/non-letter

class Example {
  int value = 0;

  void method() {
    // This is correctly formatted
    value = 1;

    // Another well-formed comment
    value = 2;

    // TODO: fix this later
    value = 3;

    // NOTE: special handling required
    print(value);
  }
}

void topLevel() {
  // Calculate the result
  final x = 1 + 2;

  // Print and return
  print(x);
}

// Empty comment line is fine
//
void anotherFunction() {
  print("ok");
}

// Comments starting with non-letter are OK
void withSpecialChars() {
  // 123 numeric start
  int count = 0;

  // @override-like patterns
  count++;

  // _underscore patterns
  count--;
}

// Uppercase variety
void uppercase() {
  // A comment
  var x = 1;

  // Z comment
  var y = 2;
}
