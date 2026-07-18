// Good: the string is more than a single bare interpolation.
String examples(String name, int n) {
  final a = 'Hello $name'; // has surrounding text
  final b = '$name!'; // trailing text
  final c = '${name}s'; // trailing text
  final d = '$name$name'; // two interpolations
  final e = 'count: ${n}'; // leading text
  final f = 'plain'; // no interpolation
  final g = r'$name'; // raw string — literal `$name`, not an interpolation
  return '$a$b$c$d$e$f$g';
}
