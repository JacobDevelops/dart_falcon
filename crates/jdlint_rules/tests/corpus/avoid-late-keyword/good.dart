// Good cases for avoid-late-keyword rule
// No violations expected

class TestClass {
  int? counter;
  String name = '';
  List<String> items = [];
  bool isActive = false;
}

void initializeWithDefaults() {
  final String message = "hello";
  final dynamicValue = null;
}

class Widget {
  BuildContext? context;

  void setup(BuildContext ctx) {
    context = ctx;
  }
}

mixin StateManagement {
  final controller = Controller();
}

int topLevel = 0;

class DelayedInit {
  String? _delayedValue;

  void initialize() {
    _delayedValue = "initialized";
  }

  String getValue() => _delayedValue ?? '';
}

class OptionalField {
  final int? maybeValue;

  OptionalField(this.maybeValue);
}
