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

class MultiField {
  // The annotation belongs to the declaration, so one diagnostic covers both.
  @deprecated /* expect: provide-deprecation-message */
  int a = 0, b = 1;
}
