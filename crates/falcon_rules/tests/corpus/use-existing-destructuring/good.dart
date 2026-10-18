int mutation((int, int) pair) {
  var (first, second) = pair;
  first = 3;
  return pair.$1;
}
int sourceMutation((int, int) pair) {
  final (first, second) = pair;
  pair = (3, 4);
  return pair.$1;
}
int otherSource((int, int) pair, (int, int) other) {
  final (first, second) = pair;
  return other.$1;
}
int scoped((int, int) pair) {
  { final (first, second) = pair; }
  return pair.$1;
}
int list(List<int> values) {
  final [first, ...rest] = values;
  return values[0];
}
class C { int get value => DateTime.now().second; }
int getter(C object) {
  final C(:value) = object;
  return object.value;
}
int captured((int, int) pair) {
  var (first, second) = pair;
  void change() { first++; }
  return pair.$1;
}
