void use(Object o) {}

void main() {
  use(r'a \ b');
  use(r'cost $5');
  use(r'\d+');
  use(r'''multi \n''');
  use('plain');
  use(r'$' 'plain');
}
