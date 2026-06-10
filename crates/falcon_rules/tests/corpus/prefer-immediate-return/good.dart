// Good: immediate return
String getName() {
  return compute();
}

int getValue() {
  return 42;
}

class Helper {
  Future<String> fetchData() async {
    return await http.get(url);
  }

  List<String> getNames(List<User> users) {
    return users.map((u) => u.name).toList();
  }
}

bool validate(String input) {
  return input.isNotEmpty && input.length > 3;
}

// OK: variable used multiple times
String processAndLog(String input) {
  final result = transform(input);
  log('Transformed: $result');
  return result;
}

// OK: variable used in conditional
int getCount(List<String> items) {
  final count = items.length;
  if (count == 0) {
    print('Empty');
  }
  return count;
}

// OK: variable needs intermediate computation
Future<String> getData(bool force) async {
  final result = await (force ? fetchRemote() : getLocal());
  return result.trim();
}
