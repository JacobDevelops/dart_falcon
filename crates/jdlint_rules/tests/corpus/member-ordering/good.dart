// Good examples for member-ordering rule
// Correct order: static const → static fields → instance fields → constructors → static methods → instance methods

class WellOrderedExample1 {
  static const String constant = "const";
  static int staticField = 10;

  final int instanceField = 42;

  WellOrderedExample1() {
    print("constructor");
  }

  static void staticMethod() {
    print("static method");
  }

  void instanceMethod() {
    print("instance method");
  }
}

class WellOrderedExample2 {
  static const int maxSize = 100;

  int count = 0;
  String name = "test";

  WellOrderedExample2() {
    count = 0;
  }

  static int defaultValue() {
    return 42;
  }

  void increment() {
    count++;
  }

  void reset() {
    count = 0;
  }
}

class WellOrderedExample3 {
  static const List<String> defaults = [];
  static bool enabled = true;

  final String id;
  late String value;

  WellOrderedExample3(this.id) {
    value = id.toUpperCase();
  }

  WellOrderedExample3.named(String name) {
    id = name;
    value = name;
  }

  static void logDefaults() {
    print("defaults");
  }

  void process() {
    print(value);
  }

  @override
  String toString() => id;
}

class SimpleClass {
  static const String defaultName = "default";

  String name;

  SimpleClass([this.name = defaultName]);

  static String getDefault() => defaultName;

  void changeName(String newName) {
    name = newName;
  }
}
