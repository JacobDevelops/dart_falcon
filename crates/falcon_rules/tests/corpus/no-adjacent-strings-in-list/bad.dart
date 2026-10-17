void use(Object o) {}

void main(bool flag, List<String> xs) {
  use(<String>[
    'a' /* expect: no-adjacent-strings-in-list */
    'b',
    'c',
  ]);
  use(['x' 'y']); /* expect: no-adjacent-strings-in-list */
  use(<String>{'set' 'element'}); /* expect: no-adjacent-strings-in-list */
  use([if (flag) 'then' 'element']); /* expect: no-adjacent-strings-in-list */
  use([if (flag) 'then' else 'else' 'element']); /* expect: no-adjacent-strings-in-list */
  use([for (var x in xs) 'body' 'part']); /* expect: no-adjacent-strings-in-list */
  switch (xs) {
    case ['pattern' 'element']: /* expect: no-adjacent-strings-in-list */
      break;
  }
}

void nestedCollectionContexts(bool flag, List<String> xs) {
  use([
    for (final x in xs)
      if (flag)
        for (final y in xs)
          'nested' 'body', /* expect: no-adjacent-strings-in-list */
  ]);
  use([
    if (flag)
      if (flag) 'innerThen' else 'inner' 'else' /* expect: no-adjacent-strings-in-list */
    else
      for (final x in xs)
        'outer' 'else', /* expect: no-adjacent-strings-in-list */
  ]);
  use([
    if (['condition' 'list'].isNotEmpty) 'body', /* expect: no-adjacent-strings-in-list */
    for (final x in ['header' 'list']) x, /* expect: no-adjacent-strings-in-list */
    ...['spread' 'list'], /* expect: no-adjacent-strings-in-list */
  ]);
}
