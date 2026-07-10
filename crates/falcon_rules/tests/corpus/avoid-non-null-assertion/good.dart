// Good cases for avoid-non-null-assertion rule
// No violations expected

void testNullCoalescing() {
  String? nullable = "test";
  final x = nullable ?? "default";
  print(x);
}

void printUserName(User? user) {
  if (user != null) {
    print(user.name);
    print(user.email);
  }
}

int getValue(int? value) {
  return (value ?? 0) + 10;
}

List<String> getList(List<String>? items) {
  return items ?? [];
}

class Widget {
  String? _title;

  String getTitle() {
    return _title ?? '';
  }

  void render() {
    final context = _context;
    if (context != null) {
      context.build();
    }
  }
}

Map<String, dynamic> parseResponse(Map<String, dynamic>? data) {
  if (data == null) return {};
  final nested = data['key'];
  return nested is Map<String, dynamic> ? nested : {};
}

Future<String> asyncOperation(Future<String>? future) {
  return future ?? Future.value('');
}

void multipleNullChecks() {
  final a = value1 ?? 'default1';
  final b = value2 ?? 'default2';
  if (nested != null && nested.property != null) {
    final c = nested.property;
  }
}

void guardClauses(String? maybeValue) {
  if (maybeValue == null) return;
  print(maybeValue);
}

// A null-assertion on a (map) index expression is exempt.
int readCount(Map<String, int> counts, List<int> xs) {
  final a = counts['key']!;
  final b = xs[0]!;
  return a + b;
}

class Optional<T> {
  final T? _value;

  Optional(this._value);

  T getOrElse(T defaultValue) => _value ?? defaultValue;

  void ifPresent(void Function(T) callback) {
    if (_value != null) callback(_value as T);
  }
}
