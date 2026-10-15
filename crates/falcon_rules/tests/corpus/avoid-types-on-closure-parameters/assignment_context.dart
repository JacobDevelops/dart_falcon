void assignments() {
  Object holder = 0;
  holder = (int value) => value; /* expect: avoid-types-on-closure-parameters */
  final entries = <String, Object>{};
  entries['next'] = (String value, int count) => value; /* expect: avoid-types-on-closure-parameters */ /* expect: avoid-types-on-closure-parameters */
  entries['dynamic'] = (dynamic value) => value;
  print([holder, entries]);
}

void voidCallback(void Function(void) callback) {}
void verify() {
  voidCallback((void value) {}); /* expect: avoid-types-on-closure-parameters */
}
