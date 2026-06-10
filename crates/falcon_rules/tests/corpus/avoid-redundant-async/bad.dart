// Bad: async with only one await and no error handling
Future<String> getName() async { /* expect: avoid-redundant-async */
  return await db.getName();
}

Future<int> getValue() async { /* expect: avoid-redundant-async */
  return await compute();
}

class Service {
  Future<User> getUser(String id) async { /* expect: avoid-redundant-async */
    return await repository.fetchUser(id);
  }

  Future<List<String>> getNames() async { /* expect: avoid-redundant-async */
    return await api.names();
  }
}

Future<dynamic> fetch(String url) async { /* expect: avoid-redundant-async */
  return await http.get(url);
}

Future<String> transform(String input) async { /* expect: avoid-redundant-async */
  return await processString(input);
}
