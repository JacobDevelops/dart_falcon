// Good: using async/await instead of .then() chains
Future<String> getData() async {
  final data = await fetch();
  return process(data);
}

void processUser() async {
  final id = await getUserId();
  print(id);
}

Future<int> compute() async {
  try {
    final d = await loadData();
    return transform(d);
  } catch (e) {
    print('Error: $e');
    return 0;
  }
}

class Api {
  Future<String> fetchName() async {
    final response = await http.get(url);
    return response.body;
  }
}

// OK: .then() with simple arrow function
Future<String> simpleFetch() => fetch().then((d) => d);
