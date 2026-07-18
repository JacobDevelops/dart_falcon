void f1() {
  ; /* expect: empty-statements */
}

void f2() {
  print('x');
  ; /* expect: empty-statements */
}

void f3(bool x) {
  while (x) ; /* expect: empty-statements */
}

void f4() {
  for (var i = 0; i < 1; i++) ; /* expect: empty-statements */
}

void f5(bool x) {
  if (x) ; /* expect: empty-statements */
}

void f6() {
  print('a');
  ; /* expect: empty-statements */
  print('b');
}
