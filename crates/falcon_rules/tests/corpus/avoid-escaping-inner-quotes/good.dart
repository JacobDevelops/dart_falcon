void use(Object o) {}

void main() {
  var name = 'Ann';
  use("It's here");
  use('Say "hi"');
  use('It\'s "both" kinds');
  use("Mixed 'single' and \"double\"");
  use(r'raw \' quote');
  use('''It\'s multiline''');
  use('plain $name');
  use("${name.contains("'")}");
}
