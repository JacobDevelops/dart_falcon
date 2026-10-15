// Positions without an expected type in the analyzer's helper.
import 'dart:async';

void use(Object? value) {}
class Box<T> {
  T? field;
}

void ifElement<T>(T? value, bool flag) {
  final List<T> list = [if (flag) value!];
  use(list);
}
void forElement<T>(T? value) {
  final List<T> list = [for (final _ in [1]) value!];
  use(list);
}
void record<T>(T? value) {
  final (T, int) pair = (value!, 1);
  use(pair);
}
void conditional<T>(T? value, T other, bool flag) {
  final T result = flag ? value! : other;
  use(result);
}
void coalesce<T>(T? value) {
  final T result = value ?? value!;
  use(result);
}
void equality<T>(T? value) {
  use(value == value!);
}
void futureValue<T>(T? value) {
  use(Future.value(value!));
}
void complete<T>(T? value) {
  Completer<T>().complete(value!);
}
void index<T>(T? value, Map<T, int> map) {
  use(map[value!]);
}
T cascadeTarget<T>(T? value) => value!..toString();
T switchArm<T>(T? value, int key) => switch (key) { _ => value! };
void patternDeclaration<T>(T? value) {
  final (first, second) = (value!, 1);
  use(first);
}
void closure<T>(T? value) {
  final T Function() callback = () => value!;
  use(callback);
}
