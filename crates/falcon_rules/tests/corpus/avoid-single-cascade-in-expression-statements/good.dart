void g1(StringBuffer a) {
  a
    ..write('x')
    ..write('y');
}

void g2(List<int> list) {
  list.add(1);
}

void g3(Foo b) {
  final x = b..field = 1;
  print(x);
}

Foo g4(Foo d) {
  return d..method();
}

void g5(StringBuffer e) {
  e
    ..clear()
    ..write('z');
}

void g6(Foo f) {
  f.method();
}
