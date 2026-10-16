// Bad: backslash escapes that do nothing in the given string context.
void examples() {
  final a = "\'"; /* expect: unnecessary-string-escapes */
  final b = '\"'; /* expect: unnecessary-string-escapes */
  final c = '\a'; /* expect: unnecessary-string-escapes */
  final d = 'x\dy'; /* expect: unnecessary-string-escapes */
  final e = "50\%"; /* expect: unnecessary-string-escapes */
  final f = '\-'; /* expect: unnecessary-string-escapes */
  print([a, b, c, d, e, f]);
  print('''one\'quote'''); /* expect: unnecessary-string-escapes */
  print("""one\"quote"""); /* expect: unnecessary-string-escapes */
  print('''opposite\"quote'''); /* expect: unnecessary-string-escapes */
  print('''unknown\q'''); /* expect: unnecessary-string-escapes */
  print('''$a\'quote'''); /* expect: unnecessary-string-escapes */
  print('''\'$a'''); /* expect: unnecessary-string-escapes */
  print('''\' 'end'''); /* expect: unnecessary-string-escapes */
}
