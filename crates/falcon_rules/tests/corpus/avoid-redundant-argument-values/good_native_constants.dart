void mapArg({Map<String, int> value = const {'a': 1, 'b': 2}}) {}
void setArg({Set<int> value = const {1, 2}}) {}
void flag({bool value = false}) {}
void doubleHex({double value = 0x8000000000000000}) {}
void doubleDecimal({double value = 9223372036854775808}) {}
void main() {
  mapArg(value: const {'b': 2, 'a': 1});
  setArg(value: const {2, 1});
  flag(value: 9007199254740992 < 9007199254740993);
  flag(value: 9007199254740993 > 9007199254740992);
  doubleHex(value: 9223372036854775808.0);
  doubleDecimal(value: -9223372036854775808.0);
}
