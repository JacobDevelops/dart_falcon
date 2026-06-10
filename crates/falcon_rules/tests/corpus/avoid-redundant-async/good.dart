// Good: removing unnecessary async/await
Future<String> getName() => db.getName();

Future<int> getValue() => compute();

class Service {
  Future<User> getUser(String id) => repository.fetchUser(id);

  Future<List<String>> getNames() => api.names();
}

Future<dynamic> fetch(String url) => http.get(url);

Future<String> transform(String input) => processString(input);

// OK: async with error handling
Future<String> safeGetName() async {
  try {
    return await db.getName();
  } catch (e) {
    return 'Default';
  }
}

// OK: async with multiple awaits
Future<User> buildUser(String id) async {
  final userData = await repository.fetchUser(id);
  final preferences = await repository.fetchPreferences(id);
  return User(userData, preferences);
}

// OK: async with additional logic
Future<String> processName() async {
  final name = await db.getName();
  print('Loaded: $name');
  return name.toUpperCase();
}
