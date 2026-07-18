// Non-constant identifiers must be lowerCamelCase.

int My_Var = 0; /* expect: non-constant-identifier-names */

void Foo() {} /* expect: non-constant-identifier-names */

void f(int Bad_Param) {} /* expect: non-constant-identifier-names */

class A {
  void Some_Method() {} /* expect: non-constant-identifier-names */

  A.My_Named(); /* expect: non-constant-identifier-names */
}

void g() {
  var Local_Var = 1; /* expect: non-constant-identifier-names */
  print(Local_Var);
}
