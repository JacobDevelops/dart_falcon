// Test cases for avoid-non-null-assertion rule
// All violations are annotated inline

void testNullAssertion() {
  String? nullable = "test";
  final x = nullable!; /* expect: avoid-non-null-assertion */
  print(x);
}

void printUserName(User? user) {
  print(user!.name); /* expect: avoid-non-null-assertion */
  print(user!.email); /* expect: avoid-non-null-assertion */
}

int getValue(int? value) {
  return value! + 10; /* expect: avoid-non-null-assertion */
}

List<String> getList(List<String>? items) {
  return items!; /* expect: avoid-non-null-assertion */
}

class Widget {
  String? _title;

  String getTitle() {
    return _title!; /* expect: avoid-non-null-assertion */
  }

  void render() {
    final context = _context!; /* expect: avoid-non-null-assertion */
    context.build();
  }
}

Map<String, dynamic> parseResponse(Map<String, dynamic>? data) {
  // The outer index `!` (`[...]!`) is exempt; only `data!` is flagged.
  final nested = data!['key']; /* expect: avoid-non-null-assertion */
  return nested;
}

Future<String> asyncOperation(Future<String>? future) {
  return future!; /* expect: avoid-non-null-assertion */
}

void multipleAssertions() {
  final a = value1!; /* expect: avoid-non-null-assertion */
  final b = value2!; /* expect: avoid-non-null-assertion */
  final c = (nested!.property)!; /* expect: avoid-non-null-assertion *//* expect: avoid-non-null-assertion */
}
