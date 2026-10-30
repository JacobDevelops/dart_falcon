import 'package:test/test.dart';
void main() {
  expect(() => Future.value(1), completes); /* expect: avoid-misused-test-matchers */
  expect(1, throwsArgumentError); /* expect: avoid-misused-test-matchers */
  expect('b', greaterThan('a')); /* expect: avoid-misused-test-matchers */
  expect(1, allOf([isPositive, isTrue])); /* expect: avoid-misused-test-matchers */
  expect(1, anyOf({isTrue, isList})); /* expect: avoid-misused-test-matchers */
  expect(1, null); /* expect: avoid-misused-test-matchers */
  expect({1}, isList); /* expect: avoid-misused-test-matchers */
  expect({1}, isNot(isList)); /* expect: avoid-misused-test-matchers */
  expect({1}, allOf(isList, isNotEmpty)); /* expect: avoid-misused-test-matchers */
  expect('one', isIn(<int>[1, 2])); /* expect: avoid-misused-test-matchers */
  expect(1, isIn(<String>{'one'})); /* expect: avoid-misused-test-matchers */
  expect(true, isNot(isIn(<int>[1]))); /* expect: avoid-misused-test-matchers */
}

void typedSet(Set<int> values) {
  expect(values, isList); /* expect: avoid-misused-test-matchers */
}
