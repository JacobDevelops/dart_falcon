void f(dynamic x) {
  if (x != null) {} else {}
  if (null != x) {} else {}
  final value = x != null ? 1 : 2;
  if (x == null) {} else {}
  if (x != 0) return;
  final list = [if (x != null) 1 else 2];
  final map = {if (null != x) 1: 2 else 3: 4};
}
