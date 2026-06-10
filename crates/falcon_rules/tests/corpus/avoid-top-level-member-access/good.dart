// Good cases for avoid-top-level-member-access rule
// No violations expected

const kTimeout = 30;

const kMaxRetries = 3;

final kDefaultConfig = <String, dynamic>{};

class Counter {
  int _value = 0;

  void increment() {
    _value++;
  }

  int get value => _value;
}

class GlobalState {
  static final _instance = GlobalState._();

  factory GlobalState() => _instance;

  GlobalState._();

  int _state = 5;

  void setState(int newValue) {
    _state = newValue;
  }
}

abstract class Repository {
  void addItem(String item);
  List<String> getItems();
}

class InMemoryRepository implements Repository {
  final _items = <String>[];

  @override
  void addItem(String item) {
    _items.add(item);
  }

  @override
  List<String> getItems() => _items;
}

class Config {
  static const String appName = 'MyApp';
  static const String version = '1.0.0';
}

const String baseUrl = 'https://api.example.com';

final RegExp emailRegex = RegExp(r'^[^@]+@[^@]+\.[^@]+$');

class Logger {
  static void log(String message) {
    print('[${DateTime.now()}] $message');
  }
}

enum Status {
  pending,
  active,
  completed,
}
