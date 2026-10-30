class Device {
  set value(int values) {
    final valuess = values; /* expect: avoid-similar-names */
  }
}

void amber(Object payload) {
  int a = 0;
  int b = 0;
  (a, b) = switch (payload) {
    (final customer, final customers) => (1, 2), /* expect: avoid-similar-names */
    _ => (0, 0),
  };
}

void birch(Object payload) {
  if (payload case final customer) {
    final customers = 1; /* expect: avoid-similar-names */
  }
}

void cedar() {
  final result = [
    for (final customer in <int>[])
      (() { final customers = 1; /* expect: avoid-similar-names */
        return customers;
      })(),
  ];
}

enum Compass {
  north,
  northe, /* expect: avoid-similar-names */
}

class Archive<Customer, Customers> {} /* expect: avoid-similar-names */

void dune(int customer) {
  int customers() => 1; /* expect: avoid-similar-names */
}

void zephyr() {
  final customer = () {
    final customers = 1; /* expect: avoid-similar-names */
    return customers;
  };
}
