// Good: using .every() for a universal predicate.
void validateItems(List<int> items) {
  if (items.every((x) => x > 0)) {
    print('All items are positive');
  }
}

// Good: .every() pattern for checking all elements
void checkAllValid(List<String> values) {
  bool allNonEmpty = values.every((v) => v.isNotEmpty);
  if (allNonEmpty) {
    print('All strings are non-empty');
  }
}

// Good: assignment with .every()
void processCollection() {
  final data = [2, 4, 6, 8];
  final allEven = data.every((n) => n % 2 == 0);
}

// Good: nested every() instead of where().length
void checkMatrix(List<List<int>> matrix) {
  if (matrix.every((row) => row.isNotEmpty)) {
    print('All rows are non-empty');
  }
}

// Good: other iterable methods are fine
void useOtherMethods(List<int> numbers) {
  final filtered = numbers.where((n) => n > 0).toList();
  final hasAny = numbers.any((n) => n > 10);
}

// Negated filtered emptiness is any(predicate), not every(predicate).
void existentialChecks(List<int> items) {
  final hasPositive = !items.where((x) => x > 0).isEmpty;
  final parenthesized = !(items.where((x) => x > 0).isEmpty);
  final nested = !((items.where((x) => x > 0).isEmpty));
  final comparedFalse = items.where((x) => x > 0).isEmpty == false;
  final comparedTrue = items.where((x) => x > 0).isEmpty != true;
  final reversedFalse = false == (items.where((x) => x > 0).isEmpty);
  final reversedTrue = true != ((items.where((x) => x > 0).isEmpty));
  final negatedComparison = !(items.where((x) => x > 0).isEmpty == true);
  final repeatedComparison = (items.where((x) => x > 0).isEmpty == false) != false;
  final allNonPositive = items.every((x) => !(x > 0));
  final empty = <int>[].every((x) => !(x > 0));
  print([hasPositive, parenthesized, nested, allNonPositive, empty]);
}

class FakeString {
  String where(bool Function(int) predicate) => '';
}

class FakeMap {
  Map<int, int> where(bool Function(int) predicate) => {};
}

void unrelatedReceivers(
  List<int> xs,
  List<int> ys,
  List<int> Function() source,
  FakeString text,
  FakeMap map,
) {
  final textEmpty = text.where((x) => x > 0).isEmpty;
  final mapEmpty = map.where((x) => x > 0).isEmpty;
  final textLength = text.where((x) => x > 0).length == xs.length;
  final mapLength = map.where((x) => x > 0).length == xs.length;
  final unrelated = xs.where((x) => x > 0).length == ys.length;
  final repeatedCall = source().where((x) => x > 0).length == source().length;
}

void nullableChains(List<int>? items, List<int> values) {
  final nullableFilter = items?.where((x) => x > 0).isEmpty;
  final nullableEmpty = values.where((x) => x > 0)?.isEmpty;
  final nullableLength =
      values.where((x) => x > 0)?.length == values.length;
  final nullableOriginal =
      values.where((x) => x > 0).length == values?.length;
}

bool unresolved(dynamic values) => values.where((value) => true).isEmpty;

class Lookalike {
  Lookalike where(bool Function(int) predicate) => this;
  bool get isEmpty => true;
}

bool lookalike(Lookalike values) => values.where((value) => true).isEmpty;
