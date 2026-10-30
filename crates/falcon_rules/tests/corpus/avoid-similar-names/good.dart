// Distinct names in one scope; uses are not declarations.
void alpha(int width, int height) {
  print(width);
  print(width);
  print(height);
}

// Sibling functions do not share their parameters or local declarations.
void bravo(int customer) {}
void charlie(int customers) {}

// Sibling blocks do not leak bindings into each other or their parent.
void delta() {
  { final customer = 1; }
  { final customers = 2; }
}

// Independent closure parameters are siblings.
void echo() {
  final first = (int customer) => customer;
  final second = (int customers) => customers;
}

// Loop declarations do not survive the loop.
void foxtrot() {
  for (final customer in <int>[1]) { print(customer); }
  final customers = 2;
}

// Catch declarations do not survive their catch clause.
void golf() {
  try { throw 1; } catch (customer) { print(customer); }
  final customers = 2;
}

// Members in unrelated types are not in overlapping scopes.
class Box { int customer = 1; }
class Jar { int customers = 2; }

// Exact shadowing is not a similarity warning.
void hotel(int customer) {
  { final customer = 1; print(customer); }
}

// Initializing formals refer to fields, rather than adding comparison names.
class Vessel {
  final int customer;
  Vessel(this.customer);
}

// Pattern assignment uses existing names, rather than declaring new names.
void india() {
  int width = 0;
  int height = 0;
  (width, height) = (1, 2);
}

// An inner binding is gone before the enclosing local is introduced.
void juliet() {
  { final customer = 1; }
  final customers = 2;
}

class Cart {
  final List<int> items = [];
  void add(int item) {}
}

final users = <int>[];
void iterate() {
  for (final user in users) { print(user); }
}

class Basket {
  final customers = <int>[];
  void add(int customer) {}
}

final planet = 1;
class Solar { int planets = 0; }
class Customer { int customers = 0; }
