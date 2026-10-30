import 'package:test/test.dart';
void main() {
  expect(1, isEmpty); /* expect: avoid-misused-test-matchers */
  expect(1, hasLength(1)); /* expect: avoid-misused-test-matchers */
  expect(1, isNull); /* expect: avoid-misused-test-matchers */
  expect(1, isNotNull); /* expect: avoid-misused-test-matchers */
  expect('yes', isTrue); /* expect: avoid-misused-test-matchers */
  expect(false, isPositive); /* expect: avoid-misused-test-matchers */
  expect(1, contains('x')); /* expect: avoid-misused-test-matchers */
  expect('text', isList); /* expect: avoid-misused-test-matchers */
  expect([1], isMap); /* expect: avoid-misused-test-matchers */
  expect(1, startsWith('x')); /* expect: avoid-misused-test-matchers */
  expect('text', everyElement(isTrue)); /* expect: avoid-misused-test-matchers */
  expect(1, returnsNormally); /* expect: avoid-misused-test-matchers */
  expect(1, completes); /* expect: avoid-misused-test-matchers */
  expect(1, emitsDone); /* expect: avoid-misused-test-matchers */
  expect(1, throwsException); /* expect: avoid-misused-test-matchers */
  expect('text', equals(1)); /* expect: avoid-misused-test-matchers */
  expect(1, allOf(isPositive, isTrue)); /* expect: avoid-misused-test-matchers */
  expect(1, anyOf(isTrue, isList)); /* expect: avoid-misused-test-matchers */
  expect(1, isNot(isTrue)); /* expect: avoid-misused-test-matchers */
  expect(1, false); /* expect: avoid-misused-test-matchers */
}
