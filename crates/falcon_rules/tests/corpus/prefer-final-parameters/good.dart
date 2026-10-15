// @dart = 3.12
class Counter {
  int count = 0;
  final String label;

  Counter(final int start, this.label) : count = start;

  Counter.named(this.count, this.label);

  void add(final int amount) {
    count += amount;
  }

  void reset(int to) {
    to = to.clamp(0, 10);
    count = to;
  }

  set value(final int v) => count = v;
}

class Child extends Counter {
  Child(super.count, super.label);
}

void greet(final String name, {final String greeting = 'Hi'}) {
  print('$greeting $name');
}

void wildcard(final int _) {}

void closures(final List<int> items) {
  items.forEach((final item) => print(item));
  void local(int x) {
    void inner() {
      x = 2;
    }

    inner();
  }

  local(1);
}

// Representation formals declare final fields, even without `final`.
extension type Identifier(int value) {}
extension type const NamedIdentifier._(int value) {}
extension type Pair((int, int) value) {}
extension type WildcardIdentifier(int _) {}
