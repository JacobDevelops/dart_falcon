class Example {
  final int value;
  Example(this.value); /* expect: newline-before-constructor */
  Example.zero() : value = 0; /* expect: newline-before-constructor */

  Example.one() : value = 1;
}

class FactoryExample {
  FactoryExample();
  factory FactoryExample.create() => FactoryExample(); /* expect: newline-before-constructor */
}

class Nested {
  void method() {
    final value = Example(1);
    print(value);
  }
  Nested(); /* expect: newline-before-constructor */
}

enum Action {
  run;
  const Action(); /* expect: newline-before-constructor */
}

class Documented {
  final int value;
  /// No blank line above this documentation.
  Documented(this.value); /* expect: newline-before-constructor */
}
