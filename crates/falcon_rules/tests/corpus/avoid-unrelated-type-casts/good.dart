import 'dart:async';

void casts(Object value, dynamic unknown, List<num> numbers, Map<Object, Object> map) {
  print(value as String);
  print(unknown as int);
  print(numbers as List<int>);
  print(numbers.cast<int>());
  print(numbers.cast<Object>());
  print(map.cast<String, int>());
}
class Custom {
  Custom cast<T>() => this;
}
void custom(Custom item) { item.cast<String>(); }

extension type Id(int value) {}
void erasedAndStructural<T>(int number, Object value, Object recordValue, Function callback, T generic, FutureOr<int> futureOr) {
  print(number as Id);
  print(value as void Function());
  print(callback as void Function(int));
  print(recordValue as (int, String));
  print((1, 2) as Object);
  print(generic as void Function());
  print(futureOr as int);
  if (value case final callback as void Function()) print(callback);
}
