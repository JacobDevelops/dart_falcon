// Bad: magic numbers not in allowed list [0, 1, 2, -1]
class Theme {
  // Bad: 16.0 is a magic number
  final padding = 16.0; /* expect: no_magic_number */

  // Bad: 0.75 is a magic number
  const threshold = 0.75; /* expect: no_magic_number */
}

void configureUI() {
  // Bad: 10 is a magic number
  int maxRetries = 10; /* expect: no_magic_number */

  // Bad: 1000 is a magic number
  int timeout = 1000; /* expect: no_magic_number */

  // Good: 0, 1, 2, -1 are allowed
  int counter = 0;
  int flag = 1;
  int pair = 2;
  int delta = -1;
}

// Bad: magic number in list literal
void processValues() {
  final values = [100, 200, 300]; /* expect: no_magic_number */ /* expect: no_magic_number */ /* expect: no_magic_number */
}

// Bad: magic number in function call
void renderWidget() {
  decorateBox(color: 0xFFFF0000); /* expect: no_magic_number */
}
