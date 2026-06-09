// Bad: unnecessary intermediate variable before return
String getName() {
  final result = compute(); /* expect: prefer-immediate-return */
  return result;
}

int getValue() {
  final value = 42; /* expect: prefer-immediate-return */
  return value;
}

class Helper {
  Future<String> fetchData() async {
    final data = await http.get(url); /* expect: prefer-immediate-return */
    return data;
  }

  List<String> getNames(List<User> users) {
    final names = users.map((u) => u.name).toList(); /* expect: prefer-immediate-return */
    return names;
  }
}

bool validate(String input) {
  final isValid = input.isNotEmpty && input.length > 3; /* expect: prefer-immediate-return */
  return isValid;
}
