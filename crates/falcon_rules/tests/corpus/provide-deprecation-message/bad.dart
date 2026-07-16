class Api {
  @deprecated /* expect: provide-deprecation-message */
  void oldMethod() {}

  @Deprecated('') /* expect: provide-deprecation-message */
  void emptyMessage() {}

  @Deprecated('   ') /* expect: provide-deprecation-message */
  void whitespaceMessage() {}

  @deprecated /* expect: provide-deprecation-message */
  int oldField = 0;
}

@deprecated /* expect: provide-deprecation-message */
void oldTopLevel() {}
