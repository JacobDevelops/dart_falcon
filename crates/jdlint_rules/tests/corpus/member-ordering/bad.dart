// Test cases for member-ordering rule
// Expected order: static const → static fields → instance fields → constructors → static methods → instance methods

class BadOrderExample1 {
  void instanceMethod() { /* expect: member-ordering */
    print("method before field");
  }

  final int instanceField = 42;
}

class BadOrderExample2 {
  final int field = 10;

  static const String constant = "const"; /* expect: member-ordering */

  int anotherField = 20;
}

class BadOrderExample3 {
  static void staticMethod() { /* expect: member-ordering */
    print("static method before instance field");
  }

  final int instanceField = 5;
}

class BadOrderExample4 {
  int field1 = 1;

  BadOrderExample4(); /* expect: member-ordering */

  int field2 = 2;
}

class BadOrderExample5 {
  void method1() {
    print("method");
  }

  static const int constant = 42; /* expect: member-ordering */

  final int field = 100;
}

class BadOrderExample6 {
  // Instance method before constructor
  void doSomething() { /* expect: member-ordering */
    print("do");
  }

  BadOrderExample6() {
    print("constructor");
  }

  final String name = "test";
}

class BadOrderExample7 {
  static int staticField = 10;

  static void staticMethod() {
    print("static method");
  }

  void instanceMethod() { /* expect: member-ordering */
    print("instance method");
  }

  static const int constant = 5; /* expect: member-ordering */
}
