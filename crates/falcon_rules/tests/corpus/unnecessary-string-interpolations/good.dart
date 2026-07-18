// Good: not a single bare interpolation, or the interpolated type is not a provable String.
String examples(String name, Object value, int n, String? maybe) {
  final a = 'Hello $name'; // has surrounding text
  final b = '$name!'; // trailing text
  final c = '${name}s'; // trailing text
  final d = '$name$name'; // two interpolations
  final e = 'count: ${n}'; // leading text
  final f = 'plain'; // no interpolation
  final g = r'$name'; // raw string — literal `$name`, not an interpolation
  final h = '${n + 1}'; // int expression — the interpolation is what makes it a String
  final i = '${value}'; // Object — the interpolation is what makes it a String
  final k = '$maybe'; // String? — nullable, must not fire
  final l = '$value'; // Object interpolation
  return '$a$b$c$d$e$f$g$h$i$k$l';
}

class ShadowedField {
  String? x;

  // A try-body local must not leak past its block: `$x` below resolves to the
  // nullable field, so proving it String via the leaked local would be wrong.
  void f() {
    try {
      var x = 'lit';
      print(x);
    } catch (_) {}
    print('$x');
  }

  // Same for a finally-body local.
  void g() {
    try {
      print('');
    } finally {
      var x = 'lit';
      print(x);
    }
    print('$x');
  }
}
