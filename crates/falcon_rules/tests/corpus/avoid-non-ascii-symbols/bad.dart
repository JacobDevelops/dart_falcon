void example(String name) {
  final accented = 'café'; /* expect: avoid-non-ascii-symbols, msg: "Replace non-ASCII characters in this string with escapes or move it to localized resources." */
  final emoji = "hello 😀"; /* expect: avoid-non-ascii-symbols */
  final cjk = '你好'; /* expect: avoid-non-ascii-symbols */
  final interpolated = 'héllo $name'; /* expect: avoid-non-ascii-symbols */
  final braced = '${name} voilà'; /* expect: avoid-non-ascii-symbols */
  final tripleSingle = '''résumé'''; /* expect: avoid-non-ascii-symbols */
  final tripleDouble = """日本語"""; /* expect: avoid-non-ascii-symbols */
  final rawSingle = r'café\n'; /* expect: avoid-non-ascii-symbols */
  final rawDouble = r"😀 $name"; /* expect: avoid-non-ascii-symbols */
  final rawTriple = r'''中文'''; /* expect: avoid-non-ascii-symbols */
  final adjacent = 'ascii' 'é' "😀"; /* expect: avoid-non-ascii-symbols */ /* expect: avoid-non-ascii-symbols */
  final nested = 'outer ${"café"}'; /* expect: avoid-non-ascii-symbols */
  final multiple = '${name} x ${name} é'; /* expect: avoid-non-ascii-symbols */
  final adjacentInterpolation = 'ascii' '${name}' 'é'; /* expect: avoid-non-ascii-symbols */
  final both = 'é ${"😀"} à'; /* expect: avoid-non-ascii-symbols */ /* expect: avoid-non-ascii-symbols */
  final multiline = /* expect: avoid-non-ascii-symbols */ '''
  café
  ''';
}
