// Bad: the whole string is a single interpolation of a provably non-nullable String.
String examples(String name) {
  final a = '$name'; /* expect: unnecessary-string-interpolations */
  final b = '${name}'; /* expect: unnecessary-string-interpolations */
  final c = "$name"; /* expect: unnecessary-string-interpolations */
  final s = 'x';
  final d = '$s'; /* expect: unnecessary-string-interpolations */
  final e = '${'lit'}'; /* expect: unnecessary-string-interpolations */
  final f = 'a' + 'b';
  final g = '$f'; /* expect: unnecessary-string-interpolations */
  return '$a$b$c$d$e$g';
}
