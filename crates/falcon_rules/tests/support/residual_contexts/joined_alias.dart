import 'package:meta/meta.dart' show awaitNotRequired;

@awaitNotRequired
typedef Callback = Future<void> Function(int value);
Future<void> longer(int value, [int? extra]) async {}
Future<void> same(int value) async {}

void check(bool flag, Callback callback, Callback? nullable) {
  final prefix = flag ? callback : longer;
  prefix(0); /* expect: discarded-futures */
  final reverse = flag ? longer : callback;
  reverse(0); /* expect: discarded-futures */
  final shape = flag ? callback : same;
  shape(0); /* expect: discarded-futures */
  final nullableShape = flag ? nullable : same;
  nullableShape!(0); /* expect: discarded-futures */
  final nullableReverse = flag ? same : nullable;
  nullableReverse!(0); /* expect: discarded-futures */
  // Separate type-use occurrences do not retain the same alias instance.
  final separate = flag ? nullable : callback;
  separate!(0); /* expect: discarded-futures */
  // Reusing the same type instance retains its annotation, including nullability.
  final identical = flag ? callback : callback;
  identical(0);
  final identicalNullable = flag ? nullable : nullable;
  identicalNullable!(0);
}

Future<void> asyncCheck(bool flag, Callback callback, Callback? nullable) async {
  final prefix = flag ? callback : longer;
  prefix(0); /* expect: unawaited-futures */
  final reverse = flag ? longer : callback;
  reverse(0); /* expect: unawaited-futures */
  final shape = flag ? callback : same;
  shape(0); /* expect: unawaited-futures */
  final nullableShape = flag ? nullable : same;
  nullableShape!(0); /* expect: unawaited-futures */
  final nullableReverse = flag ? same : nullable;
  nullableReverse!(0); /* expect: unawaited-futures */
  final separate = flag ? nullable : callback;
  separate!(0); /* expect: unawaited-futures */
  final identical = flag ? callback : callback;
  identical(0);
  final identicalNullable = flag ? nullable : nullable;
  identicalNullable!(0);
}
