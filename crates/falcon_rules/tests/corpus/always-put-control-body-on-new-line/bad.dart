void f(bool ok, List<int> xs) {
  if (ok) return; /* expect: always-put-control-body-on-new-line */

  if (ok) { print('same line'); } /* expect: always-put-control-body-on-new-line */

  if (ok) {
    print('then');
  } else print('else'); /* expect: always-put-control-body-on-new-line */

  for (var i = 0; i < 3; i++) print(i); /* expect: always-put-control-body-on-new-line */

  for (final x in xs) print(x); /* expect: always-put-control-body-on-new-line */

  while (ok) ok = false; /* expect: always-put-control-body-on-new-line */

  do print('once'); /* expect: always-put-control-body-on-new-line */
  while (false);

  do { print('block'); } /* expect: always-put-control-body-on-new-line */
  while (false);

  if (xs case [var first]) print(first); /* expect: always-put-control-body-on-new-line */
}
