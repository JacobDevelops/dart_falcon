class Matcher {}
void expect(dynamic actual, dynamic matcher) {}
Future<void> expectLater(dynamic actual, dynamic matcher) async {}
final isEmpty = Matcher();
final isNotEmpty = Matcher();
final isNull = Matcher();
final isNotNull = Matcher();
final isTrue = Matcher();
final isFalse = Matcher();
final isPositive = Matcher();
final isZero = Matcher();
final isList = Matcher();
final isMap = Matcher();
final returnsNormally = Matcher();
final completes = Matcher();
final emitsDone = Matcher();
final throwsException = Matcher();
Matcher hasLength(dynamic length) => Matcher();
Matcher contains(dynamic value) => Matcher();
Matcher startsWith(dynamic value) => Matcher();
Matcher everyElement(dynamic value) => Matcher();
Matcher equals(dynamic value) => Matcher();
Matcher isNot(dynamic value) => Matcher();
Matcher anyOf(dynamic first, [dynamic second]) => Matcher();
Matcher allOf(dynamic first, [dynamic second]) => Matcher();
Matcher isA<T>() => Matcher();
const Matcher throwsArgumentError = Matcher();
Matcher greaterThan(dynamic value) => Matcher();
Matcher isIn(Iterable<dynamic> values) => Matcher();
Matcher orderedEquals(Iterable<dynamic> values) => Matcher();
Matcher unorderedEquals(Iterable<dynamic> values) => Matcher();
Matcher containsAll(Iterable<dynamic> values) => Matcher();
Matcher anyElement(dynamic value) => Matcher();
Matcher unorderedMatches(Iterable<dynamic> values) => Matcher();
Matcher pairwiseCompare(Iterable<dynamic> values, dynamic compare, String description) => Matcher();
Matcher containsAllInOrder(Iterable<dynamic> values) => Matcher();
Matcher containsOnce(dynamic value) => Matcher();
