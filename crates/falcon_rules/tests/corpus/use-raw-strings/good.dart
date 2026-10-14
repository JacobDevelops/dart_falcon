void use(Object o) {}

void main() {
  var x = 1;
  use(r'A string with only \ and $');
  use('tab\there \$');
  use('it\'s');
  use('no escapes');
  use('\\ and $x');
  use('\n');
}
