// Good: `Object` outside field/return declarations is out of scope.
class Store {
  final Map<String, String> cache = {};

  String build() => 'x';

  int get value => 0;

  // Parameters are not checked, even when typed `Object`.
  void process(Object input) {
    final Object local = input; // locals are out of scope
    print(local);
  }

  // Setters are not return-type declarations.
  set data(Object value) {}
}

// Top-level members are out of scope.
Object topLevel() => 0;
Object globalValue = 0;
dynamic dynamicValue = 42;
