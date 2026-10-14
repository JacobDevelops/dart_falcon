class Indexed {
  @deprecated
  int operator [](int index) => 0;
  void operator []=(int index, int value) {}
}

void update(Indexed indexed) {
  indexed[0] += 1; /* expect: deprecated-member-use-from-same-package */
  indexed[0]++; /* expect: deprecated-member-use-from-same-package */
  indexed[0] = 1;
}
