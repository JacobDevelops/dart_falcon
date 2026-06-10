// Good cases for avoid-throw-in-catch-block rule
// No violations expected

void readFile() {
  try {
    final file = File('missing.txt').readAsStringSync();
  } catch (e) {
    rethrow;
  }
}

Future<String> fetchData() {
  try {
    return http.get(url);
  } catch (e, st) {
    rethrow;
  }
}

class Database {
  void connect() {
    try {
      _establishConnection();
    } catch (error) {
      rethrow;
    }
  }

  void update(String query) {
    try {
      _executeQuery(query);
    } on TimeoutException catch (e) {
      rethrow;
    } catch (e) {
      rethrow;
    }
  }
}

void parseJson(String json) {
  try {
    jsonDecode(json);
  } catch (e) {
    print('Failed to parse JSON: $e');
  }
}

void handleGracefully() {
  try {
    riskyOperation();
  } catch (e) {
    logger.error('Operation failed', error: e);
  }
}

void logAndReturn() {
  try {
    doSomething();
  } catch (e) {
    _handleError(e);
  }
}

Future<String> withFallback() {
  try {
    return fetchData();
  } catch (e) {
    return Future.value('default');
  }
}

void withCallback() {
  try {
    process();
  } catch (e) {
    onError?.call(e);
  }
}

class ErrorHandler {
  void handle(Error error) {
    try {
      _sendToServer(error);
    } catch (e) {
      logger.warn('Could not send error to server: $e');
    }
  }
}

void cleanupOnError() {
  try {
    resource.allocate();
    resource.use();
  } catch (e) {
    resource.dispose();
  }
}
