void use(Object o) {}

void main() {
  var name = 'Ann';
  use("double"); /* expect: prefer-single-quotes */
  use(r"raw double"); /* expect: prefer-single-quotes */
  use("hello $name"); /* expect: prefer-single-quotes */
  use("""multiline"""); /* expect: prefer-single-quotes */
  use('single' "double"); /* expect: prefer-single-quotes */
  use("${name.length} chars"); /* expect: prefer-single-quotes */
}
