void use(Object o) {}

void main() {
  var name = 'Ann';
  use('It\'s here'); /* expect: avoid-escaping-inner-quotes */
  use("Say \"hi\""); /* expect: avoid-escaping-inner-quotes */
  use('Don\'t panic, $name'); /* expect: avoid-escaping-inner-quotes */
  use("${name} said \"no\""); /* expect: avoid-escaping-inner-quotes */
  use('It\x27s escaped'); /* expect: avoid-escaping-inner-quotes */
  use('a\'b' 'plain'); /* expect: avoid-escaping-inner-quotes */
}
