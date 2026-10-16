import 'dart:async';

class Holder {
  final List<int> values = [];
  Holder get self => this;
  List<int> Function() get getter => () => [];
  List<int> method() => [];
  static List<int> staticMethod() => [];
}

Holder holder() => Holder();
List<int> values() => [];
typedef IntList = List<int>;

void receivers(List<int> list, Stream<int> stream, Holder box, Holder? nullable,
    List<List<int>> nested, List<List<int>>? optional,
    List<int> Function() callback, (List<int> Function(),) record) {
  list.forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  box.values.forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  box.self.values.forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  callback().forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  box.getter().forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  record.$1().forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  List<int>.of(list).forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  IntList.of(list).forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  nested[0].forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  (values()).forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  Holder().values.forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  nullable?.getter().forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  values().forEach((value) {});
  holder().values.forEach((value) {});
  holder().self.values.forEach((value) {});
  box.method().forEach((value) {});
  Holder.staticMethod().forEach((value) {});
  List.castFrom<int, int>(list).forEach((value) {});
  stream.forEach((value) {});
  nullable?.values.forEach((value) {});
  optional?[0].forEach((value) {});
  list.forEach(((value) {}));
  (list.forEach)((value) {});
  nested..forEach((values) => values.forEach((value) {}));
  nested..forEach((values) {
    values.forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  });
  nested..first.forEach((value) {});
}

extension type Values(List<int> value) implements List<int> {}

void bounded<T extends Iterable<int>>(T values, Values extensionValue) {
  values.forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
  extensionValue.forEach((value) {}); /* expect: avoid-function-literals-in-foreach-calls */
}
