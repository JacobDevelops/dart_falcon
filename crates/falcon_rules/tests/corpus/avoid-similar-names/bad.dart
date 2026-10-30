void alpha(int customer, int customers) {} /* expect: avoid-similar-names, msg: "The name `customers` is too similar to `customer`." */

void bravo() {
  final customer = 1;
  final customers = 2; /* expect: avoid-similar-names */
}

void charlie(int customer) {
  {
    final customers = 2; /* expect: avoid-similar-names */
  }
}

void delta(int customer) {
  final callback = (int customers) => customers; /* expect: avoid-similar-names */
}

void echo(int customer) {
  for (final customers in <int>[1]) { /* expect: avoid-similar-names */
    print(customers);
  }
}

void foxtrot() {
  try {
    throw 1;
  } catch (failure, failures) { /* expect: avoid-similar-names */
    print(failure);
  }
}

void golf() {
  final (customer, customers) = (1, 2); /* expect: avoid-similar-names */
}

class Basket {
  int customer = 1;
  int customers = 2; /* expect: avoid-similar-names */
}

void india() {
  final customer = 2;
  {
    final customers = 1; /* expect: avoid-similar-names */
  }
}

final planet = 1;
final planets = 2; /* expect: avoid-similar-names */
