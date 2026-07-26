// DCL does not count collection-if or collection-for elements as decisions.
List<int> collectionElements(List<int> values, bool include) => [
  if (include) 0,
  for (final value in values) value,
];
