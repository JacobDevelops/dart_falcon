// Good: valid patterns, or patterns that are not statically checkable literals.
void examples(String dynamicPattern) {
  final a = RegExp('abc');
  final b = RegExp('[a-z]+');
  final c = RegExp('(foo|bar)');
  final d = RegExp('a(b(c)d)e'); // balanced nested groups
  final e = RegExp(r'\d{3}-\d{4}'); // raw string, balanced
  final f = RegExp('\\d+'); // decoded non-raw pattern
  final g = RegExp(dynamicPattern); // not a literal, skipped
  final h = RegExp('prefix-${dynamicPattern}'); // interpolated, skipped
  final i = RegExp(r'(?=foo)foo'); // lookahead
  final j = RegExp(r'(?:foo)*?'); // non-capturing group and lazy quantifier
  final k = RegExp('[]'); // ECMAScript empty character class
  final l = RegExp('[a-]'); // trailing hyphen is a literal, not a range
  final m = RegExp(r'[a-' r'z]');
  final n = RegExp(r'\d{1,' r'3}');
  final o = RegExp('[a-' 'z]');
  final p = RegExp(r'''[a-z]+''');
  final adjacent = RegExp('(' r'\d' ')');
  print([a, b, c, d, e, f, g, h, i, j, k, l, m, n, o, p]);
  RegExp('{');
  RegExp('a{not-a-quantifier}');
  RegExp(r'\xG0');
  RegExp(r'\uZZZZ');
  RegExp(r'\q');
  RegExp(r'(?<=a)b');
  RegExp(r'(?<!a)b');
  RegExp(r'(?<word>a)\k<word>');
  RegExp(r'(?=a)+');
  RegExp(r'\p{Letter}+', unicode: true);
  RegExp(r'\u{1F600}', unicode: true);
}
