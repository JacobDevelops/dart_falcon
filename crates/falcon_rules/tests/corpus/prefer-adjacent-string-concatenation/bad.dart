// Bad: joining two string literals with `+` instead of writing them adjacent.
void examples() {
  final a = 'foo' + 'bar'; /* expect: prefer-adjacent-string-concatenation */
  final b = 'hello, ' + 'world'; /* expect: prefer-adjacent-string-concatenation */
  final c = '' + 'x'; /* expect: prefer-adjacent-string-concatenation */
  final d = 'line1\n' + 'line2'; /* expect: prefer-adjacent-string-concatenation */
  final e = "a" + "b"; /* expect: prefer-adjacent-string-concatenation */
  final f = 'value: ' + '${1}'; /* expect: prefer-adjacent-string-concatenation */
  print([a, b, c, d, e, f]);
}
