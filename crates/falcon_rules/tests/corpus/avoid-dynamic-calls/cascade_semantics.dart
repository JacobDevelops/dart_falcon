class DynamicSlot {
  dynamic value;
  dynamic operator [](int index) => null;
  void operator []=(int index, dynamic value) {}
}
void cascades(dynamic receiver, Function function, DynamicSlot slot, Invocation invocation) {
  receiver /* expect: avoid-dynamic-calls */
    ..value += 1; /* expect: avoid-dynamic-calls */
  receiver /* expect: avoid-dynamic-calls */
    ..first.second(); /* expect: avoid-dynamic-calls */
  receiver /* expect: avoid-dynamic-calls */
    ..toString();
  receiver /* expect: avoid-dynamic-calls */
    ..noSuchMethod(invocation);
  receiver..hashCode..runtimeType;
  function..call(); /* expect: avoid-dynamic-calls */
  slot..value += 1; /* expect: avoid-dynamic-calls */
  slot..[0] += 1; /* expect: avoid-dynamic-calls */
  receiver /* expect: avoid-dynamic-calls */
    ..first.toString();
  receiver /* expect: avoid-dynamic-calls */
    ..toString().contains('text');
  receiver..hashCode.isEven;
}
