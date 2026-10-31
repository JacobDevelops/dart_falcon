void example(String name) {
  // café 😀 日本語 are permitted in comments.
  final ascii = 'hello';
  final doubleQuoted = "ASCII only";
  final unicodeEscape = '\u00e9';
  final unicodeBraceEscape = '\u{1F600}';
  final escapedInterpolation = '\$name';
  final interpolated = 'hello $name';
  final braced = 'hello ${name.toUpperCase()}';
  final expressionComment = 'hello ${name /* café */}';
  final closingBrace = 'hello ${"}" /* café */}';
  final openingBrace = 'hello ${"{" /* café */}';
  final commentBrace = 'hello ${name /* } café */}';
  final multiple = '${name} ${name /* café */}';
  final adjacentInterpolation = 'prefix' '${name /* café */}';
  final nestedEscape = 'hello ${"\u00e9"}';
  final raw = r'\u00e9 $name';
  final triple = '''ASCII
only''';
  final adjacent = 'hello' " world" r'\n';
}
