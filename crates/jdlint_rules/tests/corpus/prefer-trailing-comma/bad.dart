// Bad: multi-line argument list without trailing comma
void example() {
  functionCall(
    'first',
    'second',
    'third'
  ); /* expect: prefer-trailing-comma */

  final obj = MyClass(
    fieldA,
    fieldB,
    fieldC
  ); /* expect: prefer-trailing-comma */

  anotherFunc(
    argOne,
    argTwo,
    argThree
  ); /* expect: prefer-trailing-comma */
}
