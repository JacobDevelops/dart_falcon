import 'dart:async';

abstract class Callback<T> {
  void accept(T value);
}

class StringCallback implements Callback<String> {
  @override
  void accept(value) {
    print(value.length);
  }
}

class Names extends Iterable<String> {
  @override
  Iterator<String> get iterator => <String>['ok'].iterator;

  @override
  bool any(test) => test('ok');
}

void indexedValues(List<String> list, Map<String, String> map) {
  print(list[0].length);
  print(map['key']!.length);
}

void inferredLocalReturns() {
  value() => 'known';
  print(value().length);

  blockValue() {
    return 'also known';
  }
  print(blockValue().length);
}

void externalCallChain(Stream<String> events) {
  events.asBroadcastStream().listen((event) {
    print(event.length);
  });
}

typedef StringCallbackType = void Function(String);

void typedefCallbackContext() {
  StringCallbackType callback = (value) {
    print(value.length);
  };
  callback('ok');
}
