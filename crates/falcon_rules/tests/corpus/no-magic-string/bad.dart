void f(dynamic value) {
  consume('message'); /* expect: no-magic-string */
  final instance = Model('inline'); /* expect: no-magic-string */
  if (value == 'ready') {} /* expect: no-magic-string */
  returnValue('result'); /* expect: no-magic-string */
  value.message = 'updated'; /* expect: no-magic-string */
  consume(r'raw'); /* expect: no-magic-string */
}
