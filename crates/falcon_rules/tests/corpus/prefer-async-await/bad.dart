// Bad: using .then() chains instead of async/await
Future<String> getData() {
  return fetch().then((data) { /* expect: prefer-async-await */
    return process(data);
  });
}

void processUser() {
  getUserId().then((id) { /* expect: prefer-async-await */
    print(id);
  });
}

Future<int> compute() {
  return loadData().then((d) => transform(d)).catchError((e) { /* expect: prefer-async-await */
    print('Error: $e');
    return 0;
  });
}

class Api {
  Future<String> fetchName() {
    return http.get(url).then((response) => response.body); /* expect: prefer-async-await */
  }
}
