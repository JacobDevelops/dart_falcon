// Good: valid patterns, or patterns that are not statically checkable literals.
void examples(String dynamicPattern) {
  final a = RegExp('abc');
  final b = RegExp('[a-z]+');
  final c = RegExp('(foo|bar)');
  final d = RegExp('a(b(c)d)e'); // balanced nested groups
  final e = RegExp(r'\d{3}-\d{4}'); // raw string, balanced
  final f = RegExp('\\d+'); // non-raw with escapes — pattern not resolved, skipped
  final g = RegExp(dynamicPattern); // not a literal, skipped
  final h = RegExp('prefix-${dynamicPattern}'); // interpolated, skipped
  print([a, b, c, d, e, f, g, h]);
}
