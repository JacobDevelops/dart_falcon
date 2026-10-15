extension type const Count(int value) {
  static const zero = Count(0);
  static const anotherZero = Count(0);
}

const count = Count(0); /* expect: use-named-constants */
const other = Count(1);
