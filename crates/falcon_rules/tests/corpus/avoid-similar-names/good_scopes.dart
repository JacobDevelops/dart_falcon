// Standard type/instance, backing-field, and constructor parameter conventions.
class Customer {}
class Counter {
  final int _count;
  Counter(int count) : _count = count;
  int get count => _count;
}
extension type Receipt(int receipt) {}
void amber() { final customer = Customer(); }

// Function-type names describe signatures and do not introduce bindings.
void birch(int customer, void Function(int customers) onPick) {}
typedef Callback = void Function(int customers);
typedef Legacy(int customers);
void cedar(int customer, void callback(int customers)) {}

// A class alias's type parameters do not escape into the program scope.
class Base {}
mixin Marker {}
class Wrapped<Entry> = Base with Marker;
void dune() { final Entrys = 1; }

// If-case bindings do not exist in the else branch.
void fir(Object payload) {
  if (payload case final customer) {} else { final customers = 1; }
}

// Switch alternatives have independent bindings.
void grove(Object payload) {
  switch (payload) {
    case final customer when customer is int:
      break;
    case final customers:
      break;
  }
}

// Collection loops and conditional alternatives do not leak bindings.
void harbor(Object payload) {
  final result = [
    for (final customer in <int>[]) customer,
    for (final customers in <int>[]) customers,
    if (payload case final customer) customer else (() {
      final customers = 1;
      return customers;
    })(),
  ];
}
