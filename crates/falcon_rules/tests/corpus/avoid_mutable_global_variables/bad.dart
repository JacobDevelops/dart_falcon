/// Mutable top-level variable
var items = <String>[]; /* expect: avoid_mutable_global_variables */

/// Mutable int at top-level
int count = 0; /* expect: avoid_mutable_global_variables */

/// Mutable final at top-level (can be reassigned to different object)
final RegExp _pattern = RegExp(r'[a-z]+'); /* expect: avoid_mutable_global_variables */

/// Mutable list at top-level
List<int> numbers = [1, 2, 3]; /* expect: avoid_mutable_global_variables */

/// Mutable map at top-level
Map<String, String> config = {'key': 'value'}; /* expect: avoid_mutable_global_variables */

/// Late variable without const
late String sharedState = ''; /* expect: avoid_mutable_global_variables */

class MyClass {
  /// Top-level field declarations (if not in class context)
  void example() {
    print(items);
  }
}
