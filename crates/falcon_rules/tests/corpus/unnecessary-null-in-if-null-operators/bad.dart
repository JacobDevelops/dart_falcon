void f(int? a, int? b) {
  var r1 = a ?? null; /* expect: unnecessary-null-in-if-null-operators */
  var r2 = null ?? a; /* expect: unnecessary-null-in-if-null-operators */
  var r3 = (a ?? b) ?? null; /* expect: unnecessary-null-in-if-null-operators */
  var r4 = a ?? (null); /* expect: unnecessary-null-in-if-null-operators */
  var r5 = b ?? null ?? a; /* expect: unnecessary-null-in-if-null-operators */
  print('$r1 $r2 $r3 $r4 $r5');
}
