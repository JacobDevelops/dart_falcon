// Test cases for no-empty-block rule
// All lines with violations should have /* expect: no-empty-block */

void testEmptyCatchBlock() {
  try {
    riskyOperation();
  } catch (e) {
  } /* expect: no-empty-block */
}

void testEmptyIfBlock() {
  if (condition) {
  } /* expect: no-empty-block */
}

void testEmptyElseBlock() {
  if (condition) {
    print("yes");
  } else {
  } /* expect: no-empty-block */
}

void testEmptyMethodBody() {
  onPressed() {
  } /* expect: no-empty-block */
}

class Widget {
  void onTap() {
  } /* expect: no-empty-block */

  void onLongPress() {
  } /* expect: no-empty-block */
}

void testEmptyForLoop() {
  for (int i = 0; i < 10; i++) {
  } /* expect: no-empty-block */
}

void testEmptyWhileLoop() {
  while (condition) {
  } /* expect: no-empty-block */
}

void testEmptyFinallyBlock() {
  try {
    doSomething();
  } finally {
  } /* expect: no-empty-block */
}

void testMultipleEmptyBlocks() {
  if (a) {
  } /* expect: no-empty-block */
  if (b) {
  } /* expect: no-empty-block */
}

void testEmptyOnBlock(Future<String> future) {
  future.then((_) {
  }); /* expect: no-empty-block */
}
