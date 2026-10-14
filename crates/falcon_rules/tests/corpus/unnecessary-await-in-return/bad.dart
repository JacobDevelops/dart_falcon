Future<int> fetch() async => 1;

Future<String> fetchName() async => 'name';

Future<int> arrow() async => await fetch(); /* expect: unnecessary-await-in-return */

Future<int> block() async {
  return await fetch(); /* expect: unnecessary-await-in-return */
}

Future<Object> wider() async {
  return await fetchName(); /* expect: unnecessary-await-in-return */
}

Future<dynamic> dynamicResult() async => await fetch(); /* expect: unnecessary-await-in-return */

class Repository {
  Future<int> load() async {
    return await fetch(); /* expect: unnecessary-await-in-return */
  }

  Future<int> get count async => await fetch(); /* expect: unnecessary-await-in-return */
}

void outer() {
  Future<int> local() async {
    return await fetch(); /* expect: unnecessary-await-in-return */
  }

  local();
}

Future<int> afterTry() async {
  try {
    print('x');
  } finally {
    print('y');
  }
  return await fetch(); /* expect: unnecessary-await-in-return */
}
