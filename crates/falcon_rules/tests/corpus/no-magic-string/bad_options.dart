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
}
