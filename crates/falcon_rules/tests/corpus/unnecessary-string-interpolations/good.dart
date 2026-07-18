// Good: not a single bare interpolation, or the interpolated type is not a provable String.
String examples(String name, Object value, int n, String? maybe) {
  final a = 'Hello $name'; // has surrounding text
  final b = '$name!'; // trailing text
  final c = '${name}s'; // trailing text
  final d = '$name$name'; // two interpolations
  final e = 'count: ${n}'; // leading text
  final f = 'plain'; // no interpolation
  final g = r'$name'; // raw string — literal `$name`, not an interpolation
  final h = '${n + 1}'; // int expression, not a String
  final i = '${value}'; // Object, not provably String
  final j = '${name.toUpperCase()}'; // method call, unprovable
  final k = '$maybe'; // String? — nullable, must not fire
  final l = '$value'; // Object interpolation
  return '$a$b$c$d$e$f$g$h$i$j$k$l';
}
