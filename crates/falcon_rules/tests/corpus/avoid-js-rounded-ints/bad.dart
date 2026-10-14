int a = 9007199254740993; /* expect: avoid-js-rounded-ints */
int b = 9007199254740995; /* expect: avoid-js-rounded-ints */
int c = -9007199254740993; /* expect: avoid-js-rounded-ints */
int d = 0x20000000000001; /* expect: avoid-js-rounded-ints */
int e = 1234567890123456789; /* expect: avoid-js-rounded-ints */

void use() {
  print(9007199254740997); /* expect: avoid-js-rounded-ints */
  final values = [1, 18014398509481983]; /* expect: avoid-js-rounded-ints */
  print(values);
}
