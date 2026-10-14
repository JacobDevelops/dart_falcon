class Api { /* expect: public-member-api-docs */
  Api(); /* expect: public-member-api-docs */
  int field = 0; /* expect: public-member-api-docs */
  void method() {} /* expect: public-member-api-docs */
  int get value => 0; /* expect: public-member-api-docs */
  set value(int v) {} /* expect: public-member-api-docs */
}
void function() {} /* expect: public-member-api-docs */
int variable = 0; /* expect: public-member-api-docs */
typedef Alias = int; /* expect: public-member-api-docs */
enum Choice { /* expect: public-member-api-docs */
  one, /* expect: public-member-api-docs */
  two /* expect: public-member-api-docs */
}
mixin Features { /* expect: public-member-api-docs */
  void method() {} /* expect: public-member-api-docs */
}
extension Named on int { /* expect: public-member-api-docs */
  int get doubled => this * 2; /* expect: public-member-api-docs */
}
extension type Wrapper(int value) { /* expect: public-member-api-docs */
  static void utility() {} /* expect: public-member-api-docs */
}
int get setting => 0; /* expect: public-member-api-docs */
set setting(int value) {} /* expect: public-member-api-docs */
