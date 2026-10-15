// Analyzer-confirmed AdvB4 ul_prefixed_access_name_collision.dart.
class P {
  static int? _y; /* expect: use-late-for-private-fields-and-variables */
}
class A {
  int? _y;
  void m(A other) {
    other._y!;
  }
}
