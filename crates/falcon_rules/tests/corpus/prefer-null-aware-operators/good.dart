void f(C? a, C? b) {
  var r1 = a == null ? a.field : null;
  var r2 = a == null ? null : b.field;
  var r3 = a != null ? null : a.field;
  var r4 = a == null ? 0 : a.field;
  var r5 = a?.field;
  var r6 = a == null ? null : a;
  print('$r1 $r2 $r3 $r4 $r5 $r6');
}

class C {
  int? field;
  int method() => 0;
}
