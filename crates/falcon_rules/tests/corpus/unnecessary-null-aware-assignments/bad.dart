void f(int? a, C obj, Map<String, int> m) {
  a ??= null; /* expect: unnecessary-null-aware-assignments */
  obj.field ??= null; /* expect: unnecessary-null-aware-assignments */
  m['k'] ??= null; /* expect: unnecessary-null-aware-assignments */
  _top ??= null; /* expect: unnecessary-null-aware-assignments */
  obj.nested.value ??= null; /* expect: unnecessary-null-aware-assignments */
  print('$a ${obj.field} $m');
}

int? _top;

class C {
  int? field;
  int? value;
  C get nested => this;
}
