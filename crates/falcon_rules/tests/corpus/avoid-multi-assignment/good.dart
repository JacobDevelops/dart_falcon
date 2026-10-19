void f(dynamic a, dynamic b) {
  a = 1;
  b = 1;
  a += 2;
  b ??= 3;
  a = b + 1;
  a..value = 1..other = 2;
}
