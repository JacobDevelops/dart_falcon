// Expected types of the analyzer's helper: inferred variables and
// collection elements, async returns, arguments, and assignments.
import 'dart:async';

void use(Object? value) {}
void generic<S>(S value) {}
class Box<T> {
  T? field;
}

void inferredList<T>(T? value) {
  final list = [value!]; /* expect: null-check-on-nullable-type-parameter */
  use(list);
}
void typedList<T>(T? value) {
  final List<T> list = [value!]; /* expect: null-check-on-nullable-type-parameter */
  use(list);
}
void nullableList<T>(T? value) {
  final List<T?> list = [value!]; /* expect: null-check-on-nullable-type-parameter */
  use(list);
}
void argumentList<T>(T? value) {
  use([value!]); /* expect: null-check-on-nullable-type-parameter */
}
void inferredSet<T>(T? value) {
  final set = {value!}; /* expect: null-check-on-nullable-type-parameter */
  use(set);
}
void mapValue<T>(T? value) {
  final map = {'key': value!}; /* expect: null-check-on-nullable-type-parameter */
  use(map);
}
void mapIfValue<T>(T? value, bool flag) {
  final Map<String, T> map = {if (flag) 'key': value!}; /* expect: null-check-on-nullable-type-parameter */
  use(map);
}
void inferredLocal<T>(T? value) {
  final local = value!; /* expect: null-check-on-nullable-type-parameter */
  use(local);
}
Future<void> awaitedLocal<T>(T? value) async {
  final local = await value!; /* expect: null-check-on-nullable-type-parameter */
  use(local);
}
Future<void> awaitedElement<T>(T? value) async {
  final list = [await value!]; /* expect: null-check-on-nullable-type-parameter */
  use(list);
}
Future<T> asyncReturn<T>(T? value) async {
  return value!; /* expect: null-check-on-nullable-type-parameter */
}
Future<T> awaitedReturn<T>(T? value) async {
  return await value!; /* expect: null-check-on-nullable-type-parameter */
}
FutureOr<T> asyncArrow<T>(T? value) async => value!; /* expect: null-check-on-nullable-type-parameter */
Future<List<T>> awaitedList<T>(T? value) async => await [value!]; /* expect: null-check-on-nullable-type-parameter */
Iterable<T> yielded<T>(T? value) sync* {
  yield value!; /* expect: null-check-on-nullable-type-parameter */
}
void genericArgument<T>(T? value) {
  generic(value!); /* expect: null-check-on-nullable-type-parameter */
}
void localFunction<T>(T? value) {
  T local() => value!; /* expect: null-check-on-nullable-type-parameter */
  use(local);
}
void cascadeAssignment<T>(T? value, Box<T> box) {
  box..field = value!; /* expect: null-check-on-nullable-type-parameter */
}
void functionValue<T>(T? value, T Function(T) identity) {
  use(identity(value!)); /* expect: null-check-on-nullable-type-parameter */
}
