void f() {
  consume('repeat'); /* expect: no-magic-string */
  consume('repeat'); /* expect: no-magic-string */
  consume('foo'); /* expect: no-magic-string */
  /* expect: no-magic-string */ consume('''
foo''');
}
