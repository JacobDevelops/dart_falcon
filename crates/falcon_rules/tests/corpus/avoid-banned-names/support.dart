class Base {
  Base({int? modernKey});
  void method({int? legacyArg}) {} /* expect: avoid-banned-names */
}
