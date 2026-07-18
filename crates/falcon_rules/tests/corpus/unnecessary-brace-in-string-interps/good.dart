// Good: braces are needed, or there are none to remove.
void examples(int count, String name, Map<String, int> m) {
  final a = 'total: $count'; // no braces
  final b = 'items: ${count}x'; // next char extends the identifier
  final c = 'sum: ${count + 1}'; // expression, not a simple identifier
  final d = 'field: ${m['a']}'; // index expression
  final e = 'call: ${name.length}'; // member access
  final f = 'literal text'; // no interpolation
  final g = '${count}9'; // next char is a digit — would extend
  final h = r'${notInterpolated}'; // raw string — not an interpolation
  print([a, b, c, d, e, f, g, h]);
}
