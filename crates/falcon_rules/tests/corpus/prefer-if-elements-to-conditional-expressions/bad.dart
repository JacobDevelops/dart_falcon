List<int> lists(bool flag, int a, int b) {
  return [
    1,
    flag ? a : b, /* expect: prefer-if-elements-to-conditional-expressions */
  ];
}

Set<String> sets(bool flag) {
  return {
    'x',
    flag ? 'y' : 'z', /* expect: prefer-if-elements-to-conditional-expressions */
  };
}

List<int> nestedIf(bool a, bool b) => [
  if (a) b ? 1 : 2, /* expect: prefer-if-elements-to-conditional-expressions */
];

List<int> nestedElse(bool a, bool b) => [
  if (a) 0 else b ? 1 : 2, /* expect: prefer-if-elements-to-conditional-expressions */
];

List<int> forBody(List<int> values) => [
  for (final v in values) v.isEven ? v : -v, /* expect: prefer-if-elements-to-conditional-expressions */
];

const typed = <int>[
  true ? 1 : 2, /* expect: prefer-if-elements-to-conditional-expressions */
];
