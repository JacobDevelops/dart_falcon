void f(int? a, C obj) {
  a ??= 0;
  a ??= compute();
  a = null;
  obj.field ??= 5;
  a = (a ?? 0) + 1;
  print('$a ${obj.field}');
}

int compute() => 0;

class C {
  int? field;
}
