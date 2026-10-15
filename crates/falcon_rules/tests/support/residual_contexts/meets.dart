int integer(int value) => value;
String text(String value) => value;
void functions(bool condition) {
  final Object Function(Never) combined = condition ? integer : text; /* expect: omit-local-variable-types */
  var anonymous = condition
      ? (int value) => value /* expect: avoid-types-on-closure-parameters */
      : (String value) => value; /* expect: avoid-types-on-closure-parameters */
}
