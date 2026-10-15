// @dart = 3.12
class Counter {
  int count = 0;
  final String label;

  Counter(int start, this.label) : count = start; /* expect: prefer-final-parameters */

  void add(int amount) { /* expect: prefer-final-parameters */
    count += amount;
  }

  set value(int v) => count = v; /* expect: prefer-final-parameters */

  Counter operator +(Counter other) => Counter(count + other.count, label); /* expect: prefer-final-parameters */

}

abstract class Shape {
  void resize(double factor); /* expect: prefer-final-parameters */
}

void greet(String name, {String greeting = 'Hi'}) { /* expect: prefer-final-parameters */ /* expect: prefer-final-parameters */
  print('$greeting $name');
}

void closures(List<int> items) { /* expect: prefer-final-parameters */
  items.forEach((item) => print(item)); /* expect: prefer-final-parameters */
  void local(int x) { /* expect: prefer-final-parameters */
    print(x);
  }

  local(1);
}

// Function-typed formals cannot be `final`, but the analyzer still reports them.
void callback(void cb(final int x)) { /* expect: prefer-final-parameters */
  cb(1);
}

// Ordinary constructors and methods still have rebindable parameters.
extension type Wrapper(int value) {
  Wrapper.from(int input) : this(input); /* expect: prefer-final-parameters */
  int plus(int amount) => value + amount; /* expect: prefer-final-parameters */
}
