void examples() {
  RegExp('\uD83D\uDE00+');
  RegExp('[\uD83D\uDE00-\uD83D\uDE4F]'); /* expect: valid-regexps */
  RegExp('[😀-😎]'); /* expect: valid-regexps */
  RegExp('[😀-😎]', unicode: true);
  RegExp(r'\u{41}{2}'); /* expect: valid-regexps */
  RegExp(r'[\u{41}-z]'); /* expect: valid-regexps */
  RegExp(r'[\uD83D\uDE00-\uD83D\uDE4F]'); /* expect: valid-regexps */
  RegExp(r'\u{41}{2}', unicode: true);
  RegExp(r'[\u{41}-z]', unicode: true);
  RegExp(r'[\uD83D\uDE00-\uD83D\uDE4F]', unicode: true);
  RegExp(unicode: true, '('); // SDK checks the first argument, which is named.
  RegExp(('(')); // SDK only checks direct StringLiteral arguments.
  RegExp(/* expect: valid-regexps */ '''\\
a''', unicode: true);
  RegExp('''\
a''', unicode: true);
  RegExp(r'\\u{41}{2}'); /* expect: valid-regexps */
  RegExp(r'(?<\u{1d49c}>.)');
  RegExp(r'(?<\uD835\uDC9C>.)');
  RegExp('(?<\u{1d49c}>.)');
  RegExp(r'(?<a>.)\k<\u{61}>');
  RegExp(r'\k<\u{41}{2}>'); /* expect: valid-regexps */
  RegExp(r'\k<\u{61}>(?<a>.)');
}
