// Constant identifiers must be lowerCamelCase.

const MAX_VALUE = 100; /* expect: constant-identifier-names */

const PI = 3.14; /* expect: constant-identifier-names */

const HTTP_PORT = 80; /* expect: constant-identifier-names */

enum E { FIRST, second } /* expect: constant-identifier-names */

class C {
  static const DEFAULT_SIZE = 10; /* expect: constant-identifier-names */
}

void f() {
  const LOCAL_CONST = 1; /* expect: constant-identifier-names */
  print(LOCAL_CONST);
}
