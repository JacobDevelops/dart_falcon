int mutation() {
  var x = 1;
  final cached = x + 2;
  x = 2;
  return x + 2;
}
int captured() {
  var x = 1;
  final cached = x + 2;
  void change() { x++; }
  return x + 2;
}
Object allocate() {
  final cached = Object();
  return Object();
}
int calls(int Function() next) {
  final cached = next() + 1;
  return next() + 1;
}
int shadow() {
  final x = 1;
  final cached = x + 1;
  { final x = 2; return x + 1; }
}
int branch(bool flag) {
  final x = 1;
  if (flag) { final cached = x + 1; }
  return x + 1;
}
int division() {
  final x = 1;
  final cached = x ~/ 2;
  return x ~/ 2;
}
class Counter {
  Counter operator +(int other) => Counter();
}
Counter overloaded() {
  final x = Counter();
  final cached = x + 1;
  return x + 1;
}
int mutatedCache() {
  final x = 1;
  var cached = x + 1;
  cached = 3;
  return x + 1;
}
int constantContext() {
  final cached = 2 + 3;
  const next = 2 + 3;
  return next;
}
int lateDependency() {
  late final x = 1;
  final cached = x + 1;
  return x + 1;
}
class Unstable {
  int get length => DateTime.now().second;
}
bool customGetter(Unstable value) {
  final cached = value.length.isOdd;
  return value.length.isOdd;
}
