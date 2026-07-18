// Bad: `${name}` where `name` is a simple identifier and the braces are not needed.
void examples(int count, String name) {
  final a = 'total: ${count}'; /* expect: unnecessary-brace-in-string-interps */
  final b = 'hi ${name}!'; /* expect: unnecessary-brace-in-string-interps */
  final c = '${count} items'; /* expect: unnecessary-brace-in-string-interps */
  final d = 'a ${name}.b'; /* expect: unnecessary-brace-in-string-interps */
  final e = "value=${count}"; /* expect: unnecessary-brace-in-string-interps */
  final f = 'label ${name}'; /* expect: unnecessary-brace-in-string-interps */
  print([a, b, c, d, e, f]);
}
