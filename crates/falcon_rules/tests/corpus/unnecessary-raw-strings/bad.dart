void use(Object o) {}

void main() {
  use(r'plain'); /* expect: unnecessary-raw-strings */
  use(r"double plain"); /* expect: unnecessary-raw-strings */
  use(r'''multi'''); /* expect: unnecessary-raw-strings */
  use(r''); /* expect: unnecessary-raw-strings */
  use('ok' r'plain part'); /* expect: unnecessary-raw-strings */
}
