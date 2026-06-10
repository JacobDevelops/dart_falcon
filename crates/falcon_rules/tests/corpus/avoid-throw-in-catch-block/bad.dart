// Test cases for avoid-throw-in-catch-block rule
// All violations are annotated inline

void readFile() {
  try {
    final file = File('missing.txt').readAsStringSync();
  } catch (e) {
    throw Exception('Failed to read file: $e'); /* expect: avoid-throw-in-catch-block */
  }
}

Future<String> fetchData() {
  try {
    return http.get(url);
  } catch (e, st) {
    throw Exception('Network error: $e'); /* expect: avoid-throw-in-catch-block */
  }
}

class Database {
  void connect() {
    try {
      _establishConnection();
    } catch (error) {
      throw DatabaseException('Connection failed'); /* expect: avoid-throw-in-catch-block */
    }
  }

  void update(String query) {
    try {
      _executeQuery(query);
    } on TimeoutException catch (e) {
      throw TimeoutException('Update timeout: ${e.message}'); /* expect: avoid-throw-in-catch-block */
    } catch (e) {
      throw RuntimeException(e.toString()); /* expect: avoid-throw-in-catch-block */
    }
  }
}

void parseJson(String json) {
  try {
    jsonDecode(json);
  } catch (e) {
    throw FormatException('Invalid JSON'); /* expect: avoid-throw-in-catch-block */
  }
}

void nestedTryCatch() {
  try {
    try {
      doSomething();
    } catch (e) {
      throw CustomException('Inner error'); /* expect: avoid-throw-in-catch-block */
    }
  } catch (e) {
    print('Outer handler: $e');
  }
}

Future<void> multipleThrows() {
  try {
    return riskyOperation();
  } on NotFoundException catch (e) {
    throw ApiException('Not found: ${e.message}'); /* expect: avoid-throw-in-catch-block */
  } on TimeoutException catch (e) {
    throw ApiException('Timeout: ${e.message}'); /* expect: avoid-throw-in-catch-block */
  }
}
