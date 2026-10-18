void f(dynamic x) {
  if (x != null) {} else {}
  if (null != x) {} else {}
  final value = x != null ? 1 : 2;
  if (x == null) {} else {}
  if (x != 0) return;
}
