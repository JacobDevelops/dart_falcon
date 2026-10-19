@TestOn('vm')
library;

void f() {
  consume('repeat'); /* expect: no-magic-string */
  consume('repeat'); /* expect: no-magic-string */
  consume('foo'); /* expect: no-magic-string */
  /* expect: no-magic-string */ consume('''
foo''');
  final greeting = 'hi';
  consume('hi'); /* expect: no-magic-string */
  trace('also repeated');
  consume('also repeated'); /* expect: no-magic-string */
  consume('vm'); /* expect: no-magic-string */
}

void patternOccurrence(dynamic value) {
  if (value case 'ready') {}
  consume('ready'); /* expect: no-magic-string */
  switch (value) {
    case 'switch-only':
      break;
    default:
      break;
  }
  consume('switch-only'); /* expect: no-magic-string */
}
