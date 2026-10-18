Object immediateReturn() {
  final value = Object();
  return value;
}
void twice() {
  final first = Object();
  final second = first;
  print(first);
  print(second);
}
void typed() {
  final num first = 1;
  final second = first;
  print(second);
}
void typedAlias() {
  final first = [1];
  final List<num> second = first;
  print(second);
}
void deferred() {
  late final first = Object();
  final second = first;
  print(second);
}
void separated() {
  final first = Object();
  print('intervening');
  final second = first;
  print(second);
}
void captured() {
  var first = Object();
  final second = first;
  void change() { first = Object(); }
  print(second);
}
void thrown() {
  final error = Exception();
  throw error;
}
