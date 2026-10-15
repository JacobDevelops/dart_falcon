void consume([String value = 'a\n' r'\b' "\x63"]) {}

void check() {
  consume('a\n\\bc'); /* expect: avoid-redundant-argument-values */
}
