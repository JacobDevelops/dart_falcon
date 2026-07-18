void f(int? a, int? b, C obj) {
  if (a == null) {
    a = 0;
  } else {
    a = 1;
  }
  if (a == null) {
    b = 2;
  }
  if (a != null) {
    a = 3;
  }
  if (a == null) {
    print(a);
  }
  if (a == null) {
    a = 0;
    b = 1;
  }
  a ??= 5;
  print('$a $b ${obj.field}');
}

class C {
  int? field;
}
