void use(Object o) {}

void main(bool flag, List<String> xs) {
  use(<String>['a' + 'b', 'c']);
  use(['a', 'b']);
  use([('paren' 'thesized')]);
  use({'map' 'key': 'value' 'part'});
  use([if (flag) 'then' 'quirk' else 'else']);
  use([...xs]);
  use('not' 'in a list');
  switch (xs) {
    case ['a', 'b']:
      break;
  }
}
