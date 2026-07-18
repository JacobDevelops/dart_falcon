// Bad: the whole string is a single interpolation, so the wrapping string adds nothing.
String examples(String name, Object value, int n) {
  final a = '$name'; /* expect: unnecessary-string-interpolations */
  final b = '${name}'; /* expect: unnecessary-string-interpolations */
  final c = '${value}'; /* expect: unnecessary-string-interpolations */
  final d = '${name.toUpperCase()}'; /* expect: unnecessary-string-interpolations */
  final e = "$name"; /* expect: unnecessary-string-interpolations */
  final f = '${n + 1}'; /* expect: unnecessary-string-interpolations */
  return '$a$b$c$d$e$f';
}
