// Test cases for member-ordering rule
// Expected order: static const → static fields → instance fields → constructors → static methods → instance methods

// Instance field after instance method: field is flagged (cat 2 < max 5)
class BadOrderExample1 {
  void instanceMethod() {
    print("method before field");
  }

  final int instanceField = 42; /* expect: member-ordering */
}

// Static const after instance field: constant is flagged (cat 0 < max 2)
class BadOrderExample2 {
  final int field = 10;

  static const String constant = "const"; /* expect: member-ordering */
}

// Instance field after static method: field is flagged (cat 2 < max 4)
class BadOrderExample3 {
  static void staticMethod() {
    print("static method before instance field");
  }

  final int instanceField = 5; /* expect: member-ordering */
}

// Instance field after constructor: field is flagged (cat 2 < max 3)
class BadOrderExample4 {
  int field1 = 1;

  BadOrderExample4();

  int field2 = 2; /* expect: member-ordering */
}

// Static const and instance field after instance method: both flagged
class BadOrderExample5 {
  void method1() {
    print("method");
  }

  static const int constant = 42; /* expect: member-ordering */

  final int field = 100; /* expect: member-ordering */
}

// Constructor after instance method: constructor is flagged (cat 3 < max 5)
class BadOrderExample6 {
  void doSomething() {
    print("do");
  }

  BadOrderExample6() { /* expect: member-ordering */
    print("constructor");
  }

  final String name = "test"; /* expect: member-ordering */
}

// Static const after instance method: constant is flagged (cat 0 < max 5)
class BadOrderExample7 {
  static int staticField = 10;

  static void staticMethod() {
    print("static method");
  }

  void instanceMethod() {
    print("instance method");
  }

  static const int constant = 5; /* expect: member-ordering */
}
