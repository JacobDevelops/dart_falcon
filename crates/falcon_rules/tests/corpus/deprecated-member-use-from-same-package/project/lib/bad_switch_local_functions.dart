void helper() {}

void switchScopes(int value) {
  helper();
  switch (value) {
    case 0:
      helper(); /* expect: deprecated-member-use-from-same-package */
      @deprecated
      void helper() {}
      helper(); /* expect: deprecated-member-use-from-same-package */
      break;
    case 1:
      helper();
      void helper() {}
      helper();
      break;
    default:
      helper();
  }
  helper();
}
