// OK: value used before return inside a switch case.
String pick(int code) {
  switch (code) {
    case 1:
      final v = compute();
      print(v);
      return v;
    default:
      return 'other';
  }
}

// OK: value used before return inside a local function.
int outer() {
  int helper() {
    final v = compute();
    print(v);
    return v;
  }
  return helper();
}
