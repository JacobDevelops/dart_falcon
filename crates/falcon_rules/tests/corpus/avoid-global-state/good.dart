// Good: const or immutable state
const String appName = 'MyApp';
const int maxRetries = 3;
const List<String> allowedRoles = ['admin', 'user'];

final Map<String, dynamic> config = {'version': 1};

final Duration defaultTimeout = Duration(seconds: 30);

// Good: state inside a class
class DatabaseManager {
  static final DatabaseManager _instance = DatabaseManager._();

  factory DatabaseManager() => _instance;

  DatabaseManager._();
}

class Counter {
  int count = 0;

  void increment() {
    count++;
  }
}

// Good: function-local state
void processRequest() {
  final List<String> tempList = [];
  var count = 0;
  // local variables are OK
}

// OK: @memoized pattern
class Service {
  @memoized
  Future<String> fetchData() async {
    return 'data';
  }
}
