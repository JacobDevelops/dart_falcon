int compute() => 1;
void usedInside(bool condition) {
  final value = 1;
  if (condition) { print(value); }
  print(value);
}
void effectful(bool condition) {
  final value = compute();
  if (condition) { print('hi'); }
  print(value);
}
void captured(bool condition) {
  final value = 1;
  if (condition) { print('hi'); }
  void inner() { print(value); }
  inner();
}
void conditionUse() {
  final value = true;
  if (value) { print('hi'); }
  print(value);
}
