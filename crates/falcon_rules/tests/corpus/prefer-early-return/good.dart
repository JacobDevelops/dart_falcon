void guard(bool ready) {
  if (!ready) return;
  print('ready');
  print('work');
}
void single(bool ready) {
  if (ready) { print('one'); }
}
void withElse(bool ready) {
  if (ready) { print('one'); print('two'); }
  else { print('other'); }
}
void notLast(bool ready) {
  if (ready) { print('one'); print('two'); }
  print('end');
}
int value(bool ready) {
  if (ready) { print('one'); return 1; }
  return 0;
}
void pattern(Object value) {
  if (value case int number) { print(number); print('int'); }
}
void empty(bool ready) { if (ready) {} }
void expression(bool ready) { if (ready) print('one'); }
void inferred(bool value) {
  final callback = () {
    if (value) { print('one'); print('two'); }
  };
  callback();
}
dynamic inferredReturn(bool value) {
  if (value) { print('one'); return 1; }
}
Future<void> future(bool value) async {
  if (value) { print('one'); print('two'); }
}
Iterable<int> generator(bool value) sync* {
  if (value) { yield 1; yield 2; }
}
class Value {
  int method(bool value) {
    if (value) { print('one'); return 1; }
    return 0;
  }
}
