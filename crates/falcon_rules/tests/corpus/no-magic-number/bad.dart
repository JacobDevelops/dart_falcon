// Test cases for no-magic-number. Each literal sits in a non-exempt position
// (default allow-list is [-1, 0, 1]).

// A literal in a condition is flagged.
void threshold(int count) {
  if (count > 100) { /* expect: no-magic-number */
    print(count);
  }
}

// A for-loop condition literal is flagged (the initializer is a var decl).
void loop() {
  for (var i = 0; i < 10; i++) { /* expect: no-magic-number */
    print(i);
  }
}

// A non-const constructor argument is flagged.
Widget build() {
  return SizedBox(height: 12); /* expect: no-magic-number */
}

// A non-const map's value is flagged.
Map<String, int> config() {
  return {'timeout': 5000}; /* expect: no-magic-number */
}

// Arithmetic on a magic number in an expression statement is flagged.
void compute(int value) {
  print(value * 100); /* expect: no-magic-number */
}
