void f(bool a, bool? maybe, dynamic d, Object o, int i) {
  if (a) {}
  if (!a) {}
  if (maybe == true) {}
  if (maybe != false) {}
  if (d == true) {}
  if (o == true) {}
  if (i == 1) {}
  if (a == maybe) {}
  final x = maybe ?? false;
  print(x);
}
