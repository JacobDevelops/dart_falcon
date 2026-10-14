class Compound {
  @deprecated
  int get value => 0;
  set value(int value) {}
}
void use(Compound object) {
  object.value += 1; /* expect: deprecated-member-use-from-same-package */
  object.value++; /* expect: deprecated-member-use-from-same-package */
  ++object.value; /* expect: deprecated-member-use-from-same-package */
}
