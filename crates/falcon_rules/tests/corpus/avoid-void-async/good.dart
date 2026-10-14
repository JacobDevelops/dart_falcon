import 'dart:async';

void main() async {}

Future<void> save() async {}

Future<int> compute() async => 1;

void syncWork() {}

Stream<void> events() async* {}

FutureOr<void> maybe() async {}

class Store {
  Future<void> flush() async {}

  void syncFlush() {}

  Future<void> get refresh async {}
}

void outer() {
  Future<void> inner() async {}
  final callback = () async {};
  inner();
  callback();
}
