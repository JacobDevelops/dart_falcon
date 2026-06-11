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

void moreExamples() {
  final nested = Container(
    child: Column(
      children: [1, 2, 3],
    ),
  );

  buildUI(
    param1,
    param2,
    param3
  ); /* expect: prefer-trailing-comma */
}

void oneMore() {
  final data = Provider(
    child: Text(
      'Hello World',
    ),
  );
}

void needTrailingComma() {
  configureApp(
    debug: true,
    verbose: false,
    timeout: 30
  ); /* expect: prefer-trailing-comma */
}
