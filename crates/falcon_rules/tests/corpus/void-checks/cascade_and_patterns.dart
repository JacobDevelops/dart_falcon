class Slot<T> {
  late T value;
  void put(T value) {}
  void generic<U>(U value) {}
  void operator []=(int index, T value) {}
}
void cascades(Slot<void> slot, Object event) {
  slot
    ..value = event /* expect: void-checks */
    ..put(event) /* expect: void-checks */
    ..generic<void>(event) /* expect: void-checks */
    ..[0] = event; /* expect: void-checks */
  slot..value = null..put(null);
}
void patterns(dynamic event) {
  late void value;
  [value] = [event];
  [value] = [9];
  [value] = <dynamic>[event]; /* expect: void-checks */
  (value,) = (event,); /* expect: void-checks */
  (value,) = (1,); /* expect: void-checks */
}
