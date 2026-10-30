import 'package:test/test.dart';
abstract class Animal {}
abstract class Flyer {}
class Bird extends Animal implements Flyer {}
mixin Swimmer {}
class Duck extends Animal with Swimmer {}
void check(Animal animal, Iterable<int> values) {
  expect(animal, isA<Flyer>());
  expect(animal, isA<Swimmer>());
  expect(values, isList);
  expect(1, 1.0);
  expect(1, isA<num>());
  expect(Future.value(1), completes);
  expect(() => throw ArgumentError(), throwsArgumentError);
  expect(1, anyOf([isTrue, isPositive]));
  expect(1, allOf({isZero, isPositive}));
  expect({1}, isEmpty);
  expect({1}, isNotEmpty);
  expect({1}, hasLength(1));
  expect({1}, contains(1));
  expect({1}, everyElement(isPositive));
  expect({1}, orderedEquals([1]));
  expect({1}, unorderedEquals([1]));
  expect({1}, containsAll([1]));
  expect(values, everyElement(isPositive));
  expect(values, orderedEquals([1]));
  expect(1, isIn(<num>[1, 2.0]));
  expect('one', isIn(<String>{'one'}));
  expect(1, isIn(<Object>[1, 'one']));
  expect(null, isIn(<int?>[null, 1]));
  expect(1, anyOf(isIn(<String>['one']), isPositive));
  expect({1}, anyElement(isPositive));
  expect({1}, unorderedMatches([isPositive]));
  expect({1}, pairwiseCompare([1], (int a, int b) => a == b, 'equal'));
  expect({1}, containsAllInOrder([1]));
  expect({1}, containsOnce(1));
}
