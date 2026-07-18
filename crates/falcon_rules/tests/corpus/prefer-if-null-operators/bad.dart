void f(int? a, int? b, String fallback, C obj) {
  var r1 = a == null ? b : a; /* expect: prefer-if-null-operators */
  var r2 = a != null ? a : b; /* expect: prefer-if-null-operators */
  var r3 = null == a ? b : a; /* expect: prefer-if-null-operators */
  var r4 = a == null ? 0 : a; /* expect: prefer-if-null-operators */
  var r5 = obj.name == null ? fallback : obj.name; /* expect: prefer-if-null-operators */
  print('$r1 $r2 $r3 $r4 $r5');
}

class C {
  String? name;
}
