void forwardReference() {
  old(); /* expect: deprecated-member-use-from-same-package */
  @deprecated
  void old() {}
}
