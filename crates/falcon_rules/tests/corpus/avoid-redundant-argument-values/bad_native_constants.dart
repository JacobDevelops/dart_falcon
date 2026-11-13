void mapArg({Map<String, int> value = const {'a': 1, 'b': 2}}) {}
void setArg({Set<int> value = const {1, 2}}) {}
void flag({bool value = false}) {}
void integer({int value = -1}) {}
void doubleHex({double value = 0x8000000000000000}) {}
void doubleDecimal({double value = 9223372036854775808}) {}
void doubleBeyondWord({double value = 0x10000000000000000}) {}
void main() {
  mapArg(value: const {'a': 1, 'b': 2}); /* expect: avoid-redundant-argument-values */
  setArg(value: const {1, 2}); /* expect: avoid-redundant-argument-values */
  flag(value: 9007199254740992 >= 9007199254740993); /* expect: avoid-redundant-argument-values */
  integer(value: 0xffffffffffffffff); /* expect: avoid-redundant-argument-values */
  integer(value: 0xffffffffffffffff + 0); /* expect: avoid-redundant-argument-values */
  doubleHex(value: -9223372036854775808.0); /* expect: avoid-redundant-argument-values */
  doubleDecimal(value: 9223372036854775808.0); /* expect: avoid-redundant-argument-values */
  doubleBeyondWord(value: 18446744073709551616.0); /* expect: avoid-redundant-argument-values */
}
