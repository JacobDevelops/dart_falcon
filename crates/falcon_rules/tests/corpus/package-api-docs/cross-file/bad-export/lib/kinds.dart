mixin Features { /* expect: package-api-docs */
  void method() {} /* expect: package-api-docs */
}
enum Choice { /* expect: package-api-docs */
  one /* expect: package-api-docs */
}
extension Named on int { /* expect: package-api-docs */
  int get doubled => this * 2; /* expect: package-api-docs */
}
/// Wrapper.
extension type Wrapper(int value) { /* expect: package-api-docs */
  static void utility() {} /* expect: package-api-docs */
}
int get setting => 0; /* expect: package-api-docs */
set setting(int value) {} /* expect: package-api-docs */
