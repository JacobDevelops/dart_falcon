void use(Object o) {}

void main() {
  var name = "Ann";
  use('single'); /* expect: prefer-double-quotes */
  use(r'raw single'); /* expect: prefer-double-quotes */
  use('hello $name'); /* expect: prefer-double-quotes */
  use('''multiline'''); /* expect: prefer-double-quotes */
  use("double" 'single'); /* expect: prefer-double-quotes */
  use('${name.length} chars'); /* expect: prefer-double-quotes */
}
