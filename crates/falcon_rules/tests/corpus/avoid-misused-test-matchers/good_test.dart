import 'package:test/test.dart';
void examples(dynamic unknown, Object object, int? maybe) {
  expect('', isEmpty);
  expect([1], hasLength(1));
  expect(1, isPositive);
  expect(true, isTrue);
  expect([1], contains(1));
  expect({'a': 1}, isMap);
  expect([1], isList);
  expect('text', startsWith('t'));
  expect([true], everyElement(isTrue));
  expect(() {}, returnsNormally);
  expect(Future.value(1), completes);
  expect(Stream.empty(), emitsDone);
  expect(() { throw 'error'; }, throwsException);
  expect(maybe, isNull);
  expect(maybe, isNotNull);
  expect(unknown, isEmpty);
  expect(object, isTrue);
  expect(1, anyOf(isTrue, isPositive));
  expect('text', equals('text'));
  final isEmpty = true;
  expect(1, isEmpty);
}
