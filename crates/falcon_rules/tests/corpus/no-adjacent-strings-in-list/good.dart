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

void collectionContextsDoNotLeak(bool flag, List<String> xs) {
  use([
    if (('condition' 'text').isNotEmpty) 'body',
    for (final x in ('header' 'text').split('')) x,
    ...('spread' 'text').split(''),
    if (flag)
      'outerThen' 'excluded'
    else
      if (flag) 'innerThen' else 'innerElse',
  ]);
  use([
    if (flag) 'excluded' 'then' else 'included',
    for (final x in xs) ('parenthesized' 'body'),
  ]);
  use('outside' 'collection');
}
