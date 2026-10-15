// @dart = 3.6
// Before Dart 3.7 `_` is an ordinary parameter name, so it is checked too.
void ignore(int _) {} /* expect: prefer-final-parameters */
void both(String value, int _) => print(value); /* expect: prefer-final-parameters */ /* expect: prefer-final-parameters */
