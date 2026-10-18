void f(bool a, bool b, Object? value) {
  if (a && b) {}
  if (a) { print(a); if (b) {} }
  if (a) { if (b) {} else {} }
  if (a) { if (b) {} } else {}
  if (value case int n) { if (n > 0) {} }
  if (a) { if (value case int n) { print(n); } }
}
