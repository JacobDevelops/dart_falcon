// Test cases for avoid-late-keyword rule
// All lines with violations have the expect annotation at end of line.

class TestClass {
  late int counter; /* expect: avoid-late-keyword */
  late String name; /* expect: avoid-late-keyword */
  late List<String> items; /* expect: avoid-late-keyword */
  late bool isActive; /* expect: avoid-late-keyword */
}

void initializeWithLate() {
  late final String message = "hello"; /* expect: avoid-late-keyword */
  late var dynamicValue; /* expect: avoid-late-keyword */
}

class Widget {
  late BuildContext context; /* expect: avoid-late-keyword */

  void setup(BuildContext ctx) {
    context = ctx;
  }
}

mixin StateManagement {
  late final controller = Controller(); /* expect: avoid-late-keyword */
}

late int topLevel = 0; /* expect: avoid-late-keyword */
