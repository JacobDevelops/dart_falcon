// Good: using specific types instead of Object
void example() {
  final Map<String, dynamic> data = fetchData();
  final List<String> result = [];
  final String? nullable = null;
  final int count = 0;
}

class Store {
  final Map<String, String> cache = {};

  void process(String input) {
    print(input);
  }
}

dynamic dynamicValue = 42; // OK: dynamic is allowed, just not Object
