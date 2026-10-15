import 'dart:async';

dynamic config;

void allowed(dynamic value) {
  print(value.hashCode);
  print(value.runtimeType);
  print(value.toString());
  print(value == 1);
  print(value != null);
  value!;
  print(value ?? 1);
}

void casts(dynamic value) {
  (value as dynamic).foo();
  (value as String).length;
  value = 2;
}

void typed(String text, List<int> numbers, Map<String, int> counts) {
  print(text.length);
  print(numbers.first.isEven);
  print(counts['a']);
}

void untypedClosure(List<String> names) {
  names.forEach((name) => print(name.length));
  final upper = names.map((name) => name.toUpperCase());
  print(upper);
}

void untypedLiteral() {
  final values = [1, 2];
  print(values.first.isEven);
}

class Holder {
  String config = '';

  void read() {
    print(config.length);
  }
}

void functionType(void Function() callback, int Function(int) mapper) {
  callback();
  print(mapper(1));
}

void shadowed(String value) {
  print(value.length);
}

void otherShadowed(dynamic value) {
  print(value.hashCode);
}

void promotedMapAndString(dynamic data) {
  if (data is Map) {
    print(data['key']);
  }
  if (data is String) {
    print(data.length);
  }
}

void promotedByEarlyReturn(dynamic list) {
  if (list is! List) return;
  print(list.length);
}

void collectionForShadow(dynamic item, List<String> items) {
  print([for (final String item in items) item.length]);
}

void acceptsTypedCallback(void Function(String) callback) {}

void contextualParameters() {
  acceptsTypedCallback((value) => print(value.length));
  void Function(String) callback = (value) => print(value.length);
  callback('text');
}

void unresolvedExternalContext(Stream<String> events) {
  events.listen((event) => print(event.length));
}

void acceptsNamedCallback(void Function({required String value}) callback) {}

void namedContextualParameters() {
  acceptsNamedCallback(({required value}) => print(value.length));
}

void acceptsGenericCallback(void Function<T>(T value) callback) {}

void genericContextualParameters() {
  acceptsGenericCallback(<T>(value) {
    T typed = value;
    print(typed);
  });
}

void cascadeCallback(List<int> values) {
  values..sort((left, right) => left.compareTo(right));
}

void nullCoalescingCallback() {
  void Function(String)? callback;
  callback ??= (value) => print(value.length);
  callback('text');
}

void explicitlyTypedClosureCollections() {
  final callbacks = <void Function(String)>[
    (value) => print(value.length),
  ];
  final callbackSet = <void Function(String)>{
    (value) => print(value.length),
  };
  final callbackMap = <String, void Function(Map<String, int>)>{
    'count': (value) => print(value.length),
  };
  print([callbacks, callbackSet, callbackMap]);
}

void contextuallyTypedClosureCollections() {
  List<void Function(String)> callbacks = [
    (value) => print(value.length),
  ];
  Map<String, void Function(String)> callbackMap = {
    'text': (value) => print(value.length),
  };
  print([callbacks, callbackMap]);
}

void explicitlyCastFunctionCascade(Object callback) {
  (callback as Function)..call();
}
