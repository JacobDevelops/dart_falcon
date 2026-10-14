void use(Object o) {}

void main() {
  /* expect: lines-longer-than-80-chars */ // A plain comment that keeps going and going well past the eighty limit.
  use('A single-line string literal that is long but has no path separators.'); /* expect: lines-longer-than-80-chars */
  var aVeryLongVariableNameThatJustKeepsGoing = 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8; /* expect: lines-longer-than-80-chars */
  use(aVeryLongVariableNameThatJustKeepsGoing);
  /* Block comment text that is very long and keeps going past the limit. */ /* expect: lines-longer-than-80-chars */
  use('héllo wörld with accents ééééééééééééééééééééééééééééééééééééééééé'); /* expect: lines-longer-than-80-chars */
}
