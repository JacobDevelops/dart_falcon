void use(void Function(int) callback) {}

void fallbacks(void Function(int)? callback) {
  use(callback ?? ((int value) {}));
  use(callback ?? (((int value) {})));
  use((int value) {}); /* expect: avoid-types-on-closure-parameters */
  use(callback ?? ((int value) {
    use((int nested) {}); /* expect: avoid-types-on-closure-parameters */
  }));
}
