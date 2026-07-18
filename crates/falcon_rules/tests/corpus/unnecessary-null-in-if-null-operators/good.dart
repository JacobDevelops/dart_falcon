void f(int? a, int? b, int c) {
  var r1 = a ?? b;
  var r2 = a ?? c;
  var r3 = a ?? b ?? c;
  var r4 = a == null ? b : a;
  var r5 = a! + c;
  print('$r1 $r2 $r3 $r4 $r5');
}
