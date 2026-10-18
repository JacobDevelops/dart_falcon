void f(bool a, bool b, List<bool> items) {
  while (a) { break; }
  while (a && b) { break; }
  do {} while (a || b);
  for (var i = 0; i < 2; i++) {}
  for (final item in items) { if (item && a && b) {} }
  for (;;) { break; }
}
