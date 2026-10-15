void use(Object o) {}

void main() {
  /* expect: lines-longer-than-80-chars */ // A plain comment that keeps going and going well past the eighty limit.
  use('A single-line string literal that is long but has no path separators.'); /* expect: lines-longer-than-80-chars */
  var aVeryLongVariableNameThatJustKeepsGoing = 1 + 2 + 3 + 4 + 5 + 6 + 7 + 8; /* expect: lines-longer-than-80-chars */
  use(aVeryLongVariableNameThatJustKeepsGoing);
  /* Block comment text that is very long and keeps going past the limit. */ /* expect: lines-longer-than-80-chars */
  use('héllo wörld with accents ééééééééééééééééééééééééééééééééééééééééé'); /* expect: lines-longer-than-80-chars */
}

var nestedPath = '${"p/q"} xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; /* expect: lines-longer-than-80-chars */

const kIsWeb = true;
var bindingMessage =
        'The app requested a view, but the platform did not provide one.\n'
        'Try using `runWidget` instead of `runApp` to start your app.\n'
        '`runWidget` allows you to provide a `View` widget, without requiring '
        'a default view.'
        '${kIsWeb ? "\nSee: https://flutter.dev/to/web-multiview-runwidget" : ""}'; /* expect: lines-longer-than-80-chars */

void patternStrings(Object value) {
  switch (value) {
    case '${"a/b"} xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx': /* expect: lines-longer-than-80-chars */
    case '${"${"a/b"}"} xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx': /* expect: lines-longer-than-80-chars */
      break;
  }
}
