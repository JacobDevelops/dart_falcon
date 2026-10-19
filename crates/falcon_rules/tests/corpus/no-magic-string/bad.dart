void f(dynamic value) {
  consume('message'); /* expect: no-magic-string */
  final instance = Model('inline'); /* expect: no-magic-string */
  if (value == 'ready') {} /* expect: no-magic-string */
  returnValue('result'); /* expect: no-magic-string */
  value.message = 'updated'; /* expect: no-magic-string */
  consume(r'raw'); /* expect: no-magic-string */
  if ('admin'.contains(value)) {} /* expect: no-magic-string */
  'a-b'.split('-'); /* expect: no-magic-string */
  value.contains(User(name: 'admin')); /* expect: no-magic-string */
  print(User(name: 'nested')); /* expect: no-magic-string */
  final s = '${lookup("magic-key")}'; /* expect: no-magic-string */
}
