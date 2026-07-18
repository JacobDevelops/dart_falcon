/// Mutable top-level variable
var items = <String>[]; /* expect: avoid-mutable-global-variables */

/// Mutable int at top-level
int count = 0; /* expect: avoid-mutable-global-variables */

/// Mutable list at top-level
List<int> numbers = [1, 2, 3]; /* expect: avoid-mutable-global-variables */

/// Mutable map at top-level
Map<String, String> config = {'key': 'value'}; /* expect: avoid-mutable-global-variables */

/// Late top-level variable that is neither final nor const
late String sharedState; /* expect: avoid-mutable-global-variables */

class MyClass {
  void example() {
    print(items);
  }
}
