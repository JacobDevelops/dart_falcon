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
