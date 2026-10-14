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
