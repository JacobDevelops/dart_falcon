void f(bool ok, List<int> xs) {
  if (ok)
    return;

  if (ok) {
    print('then');
  } else if (xs.isEmpty) {
    print('else if is not reported at its else');
  } else {
    print('else');
  }

  if (ok) {}

  for (var i = 0; i < 3; i++)
    print(i);

  for (final x in xs) {
    print(x);
  }

  while (ok) {
    ok = false;
  }

  do {
    print('once');
  } while (false);

  if (ok) // comment
    print('body on the next line');
}
