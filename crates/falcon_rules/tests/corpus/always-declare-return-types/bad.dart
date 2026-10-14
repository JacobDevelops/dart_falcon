main() {} /* expect: always-declare-return-types */

get answer => 42; /* expect: always-declare-return-types */

typedef Handler(int x); /* expect: always-declare-return-types */

class Foo {
  bar() => 1; /* expect: always-declare-return-types */
  static make() => Foo(); /* expect: always-declare-return-types */
  get size => 0; /* expect: always-declare-return-types */
  operator +(Foo other) => this; /* expect: always-declare-return-types */
}

void outer() {
  inner() {} /* expect: always-declare-return-types */
  inner();
}

extension on int {
  doubled() => this * 2; /* expect: always-declare-return-types */
}
