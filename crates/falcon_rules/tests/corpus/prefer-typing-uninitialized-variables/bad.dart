// Uninitialized variables without a type annotation.
var topLevel; /* expect: prefer-typing-uninitialized-variables */

void f() {
  var a; /* expect: prefer-typing-uninitialized-variables */
  var b; /* expect: prefer-typing-uninitialized-variables */
  late var d; /* expect: prefer-typing-uninitialized-variables */
  a = 1;
  b = 2;
  d = 3;
  print(a + b + d);
}

class C {
  var field; /* expect: prefer-typing-uninitialized-variables */
  static var s; /* expect: prefer-typing-uninitialized-variables */
}
