void f(C? a, C? b) {
  var r1 = a == null ? null : a.field; /* expect: prefer-null-aware-operators */
  var r2 = a != null ? a.field : null; /* expect: prefer-null-aware-operators */
  var r3 = a == null ? null : a.method(); /* expect: prefer-null-aware-operators */
  var r4 = null == a ? null : a.field; /* expect: prefer-null-aware-operators */
  var r5 = b == null ? null : b.method(); /* expect: prefer-null-aware-operators */
  print('$r1 $r2 $r3 $r4 $r5');
}

class C {
  int? field;
  int method() => 0;
}
