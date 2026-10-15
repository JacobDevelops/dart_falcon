void plain([String value = 'abc']) {}
void leadingLine([String value = '\nabc']) {}

void checkMultiline() {
  plain(/* expect: avoid-redundant-argument-values */ '''
abc''');
  plain(/* expect: avoid-redundant-argument-values */ r''' 	
abc''');
  leadingLine('''
abc''');
  plain(/* expect: avoid-redundant-argument-values */ '''\
abc''');
  plain(/* expect: avoid-redundant-argument-values */ r'''\
abc''');
  leadingLine('''\
abc''');
  leadingLine(r'''\
abc''');
}
