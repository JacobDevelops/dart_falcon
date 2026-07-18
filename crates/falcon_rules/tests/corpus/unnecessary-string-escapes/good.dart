// Good: every backslash escape is meaningful (or the string is raw).
void examples() {
  final a = 'it\'s'; // escaping the delimiter quote is required
  final b = "she said \"hi\""; // escaping the delimiter quote is required
  final c = 'line1\nline2'; // \n is a newline
  final d = 'tab\there'; // \t is a tab
  final e = 'cost: \$5'; // \$ escapes interpolation
  final f = 'back\\slash'; // \\ is a literal backslash
  final g = r'\d+\w*'; // raw string — backslashes are literal, never flagged
  final h = 'emoji \u{1F600}'; // \u is a unicode escape
  final i = 'plain text';
  print([a, b, c, d, e, f, g, h, i]);
}
