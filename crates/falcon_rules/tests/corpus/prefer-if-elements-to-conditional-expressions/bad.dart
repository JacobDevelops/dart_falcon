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

List<int> nestedElseIf(bool a, bool b, bool c, bool d, bool e) => [
  if (a)
    b ? 1 : 2 /* expect: prefer-if-elements-to-conditional-expressions */
  else if (c)
    d ? 3 : 4 /* expect: prefer-if-elements-to-conditional-expressions */
  else
    e ? 5 : 6, /* expect: prefer-if-elements-to-conditional-expressions */
];

List<int> ifInsideFor(List<int> values, bool a, bool b, bool c) => [
  for (final value in values)
    if (a)
      b ? value : -value /* expect: prefer-if-elements-to-conditional-expressions */
    else
      c ? value : 0, /* expect: prefer-if-elements-to-conditional-expressions */
];
