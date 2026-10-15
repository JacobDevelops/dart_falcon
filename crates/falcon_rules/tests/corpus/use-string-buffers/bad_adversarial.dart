// Analyzer-confirmed AdvB4: prefix matching unwraps interpolation parentheses.
void compose(List<String> xs) {
  var s = '';
  for (var x in xs) {
    s = '${(s)}x'; /* expect: use-string-buffers */
  }
}
