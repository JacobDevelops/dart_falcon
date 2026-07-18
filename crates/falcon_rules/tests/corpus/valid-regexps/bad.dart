// Bad: literal patterns that are structurally invalid regular expressions.
void examples() {
  final a = RegExp('('); /* expect: valid-regexps */
  final b = RegExp(')'); /* expect: valid-regexps */
  final c = RegExp('[a-z'); /* expect: valid-regexps */
  final d = RegExp('(abc'); /* expect: valid-regexps */
  final e = RegExp('a(b(c)'); /* expect: valid-regexps */
  final f = new RegExp('foo)'); /* expect: valid-regexps */
  print([a, b, c, d, e, f]);
}
