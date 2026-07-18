void f(int? a, int? b, C obj) {
  var r1 = a == null ? a : b;
  var r2 = a == null ? b : b;
  var r3 = a != null ? b : a;
  var r4 = a! > 0 ? a : b;
  var r5 = obj.name == null ? a : obj.other;
  var r6 = a ?? b;
  print('$r1 $r2 $r3 $r4 $r5 $r6');
}

class C {
  int? name;
  int? other;
}
