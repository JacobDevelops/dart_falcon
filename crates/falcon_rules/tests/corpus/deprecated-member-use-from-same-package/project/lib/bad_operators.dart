class OldOperators {
  @deprecated
  OldOperators operator +(Object other) => this;
  @deprecated
  OldOperators operator -(Object other) => this;
  OldOperators operator -() => this;
  @deprecated
  int operator [](int index) => 0;
  @deprecated
  void operator []=(int index, int value) {}
  @deprecated
  bool operator ==(Object other) => false;
  int get hashCode => 0;
}
void operators(OldOperators value) {
  value + 1; /* expect: deprecated-member-use-from-same-package */
  value - 1; /* expect: deprecated-member-use-from-same-package */
  value[0]; /* expect: deprecated-member-use-from-same-package */
  value[0] = 1; /* expect: deprecated-member-use-from-same-package */
  value == 1; /* expect: deprecated-member-use-from-same-package */
  value++ ; /* expect: deprecated-member-use-from-same-package */
  -value;
  value == null;
}
