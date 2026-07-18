void f(int? a, C obj) {
  if (a == null) { a = 0; } /* expect: prefer-conditional-assignment */
  if (a == null) a = 1; /* expect: prefer-conditional-assignment */
  if (null == a) { a = 2; } /* expect: prefer-conditional-assignment */
  if (obj.field == null) { obj.field = 3; } /* expect: prefer-conditional-assignment */
  if (obj.field == null) obj.field = 4; /* expect: prefer-conditional-assignment */
  print('$a ${obj.field}');
}

class C {
  int? field;
}
