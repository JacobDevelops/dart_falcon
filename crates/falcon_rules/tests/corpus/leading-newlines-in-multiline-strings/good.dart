void use(Object o) {}

void main() {
  var name = 'Ann';
  use('''
{
  "a": 1
}''');
  use('''single line''');
  use("""
second""");
  use(r'''
raw''');
  use('''
$name''');
  use('not multiline');
}
