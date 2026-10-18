void f(bool flag, dynamic value) {
  if (flag) {} else {}
  if (!flag) return;
  if (value == 1) {} else {}
  if (value is String) {} else {}
  final a = flag ? 1 : 2;
  if (!flag) {} else if (value == 1) {}
}
