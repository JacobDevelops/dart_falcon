void inferredCollectionTypes(int? nullableInt) {
  for (final n in ['a', 'b']) {
    print(n.length);
  }
  for (final n in {'a', 'b'}) {
    print(n.length);
  }
  final strings = ['a', 'b'];
  print(strings.first.length);
  print(strings[0].length);
  final numbers = [1, 2.5];
  print(numbers.first.round());
  final nullableNumbers = [nullableInt, 1];
  print(nullableNumbers.first?.isEven);
  final nullableStrings = ['a', null];
  print(nullableStrings.first?.length);
  final counts = {'a': 1, 'b': 2.5};
  print(counts['a']?.round());
  final nullableValues = {'a': 'value', 'b': null};
  print(nullableValues['a']?.length);
  final keys = {null: 'value', 1: 'other'};
  print(keys[1]?.length);
  final objects = [true, 1];
  print(objects.first.toString());
  final nullableObjects = [null, true, 1];
  print(nullableObjects.first?.toString());
}

void contextualCollectionTypes() {
  final List<String> strings = [];
  final Set<String> names = {};
  final Map<String, String> values = {};
  print(strings.first.length);
  print(names.first.length);
  print(values['a']?.length);
  final explicitStrings = <String>[];
  final explicitNames = <String>{};
  final explicitValues = <String, String>{};
  print(explicitStrings.first.length);
  print(explicitNames.first.length);
  print(explicitValues['a']?.length);
}
