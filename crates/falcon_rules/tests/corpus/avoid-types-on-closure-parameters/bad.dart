int apply(int Function(int) fn) => fn(1);

void each(List<String> names, void Function(String name) action) {}

void entries(void Function(String key, int value) action) {}

void typed(void Function(void Function() inner) action) {}

void f(List<String> names) {
  apply((int x) => x + 1); /* expect: avoid-types-on-closure-parameters */
  each(names, (String name) { /* expect: avoid-types-on-closure-parameters */
    print(name);
  });
  entries((String key, int value) {}); /* expect: avoid-types-on-closure-parameters */ /* expect: avoid-types-on-closure-parameters */
  entries((key, int value) {}); /* expect: avoid-types-on-closure-parameters */
  typed((void inner()) {}); /* expect: avoid-types-on-closure-parameters */
  final int Function(int) inc = (int x) => x + 1; /* expect: avoid-types-on-closure-parameters */
  print(inc);
}
