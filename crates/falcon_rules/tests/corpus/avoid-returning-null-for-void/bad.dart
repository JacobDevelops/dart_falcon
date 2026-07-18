void f() {
  return null; /* expect: avoid-returning-null-for-void */
}

void g() => null; /* expect: avoid-returning-null-for-void */

Future<void> h() async {
  return null; /* expect: avoid-returning-null-for-void */
}

class C {
  void method() {
    return null; /* expect: avoid-returning-null-for-void */
  }

  set value(int v) {
    return null; /* expect: avoid-returning-null-for-void */
  }
}

void outer() {
  void inner() {
    return null; /* expect: avoid-returning-null-for-void */
  }

  inner();
}
