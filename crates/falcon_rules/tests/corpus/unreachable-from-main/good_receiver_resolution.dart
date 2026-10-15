import 'dart:async';

void main() {
  final box = Box();
  print(box + 1);
  print(box[0]);
  print(-box);
  print(box.extra());
  box.counter += 1;
  print(LocalIterable());
  readAsyncFields();
}

class Box {
  int counter = 0;
  Box operator +(int value) => this;
  int operator [](int index) => index;
  Box operator -() => this;
}

extension Extra on Box {
  int extra() => 1;
}

class LocalIterable extends Iterable<int> {
  @override
  Iterator<int> get iterator => [1].iterator;
}

class AsyncFields {
  final String? first;
  final int? second;
  AsyncFields({this.first, this.second});
}

Future<T> resolveAsync<T extends Object>(T value) async => value;

Future<void> readAsyncFields() async {
  final value = await resolveAsync<AsyncFields>(
    AsyncFields(first: 'value', second: 3),
  );
  // Awaiting a generic SDK Future must retain the instance's field owner.
  print(value.first);
  print(value.second);
}
