void example() {
  var legacyValue = 1; /* expect: avoid-banned-names */
  print(legacyValue);
}

class legacyClass { /* expect: avoid-banned-names */
  int legacyField = 0; /* expect: avoid-banned-names */
  void legacyMethod() {} /* expect: avoid-banned-names */
}
