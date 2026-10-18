int different(bool ready) {
  if (ready) return 1;
  return 2;
}
String whitespace(bool ready) {
  if (ready) return 'a b';
  return 'ab';
}
int work(bool ready) {
  if (ready) { print('work'); return 1; }
  return 1;
}
int otherwise(bool ready) {
  if (ready) return 1; else return 2;
}
int intervening(bool ready) {
  if (ready) return 1;
  print('work');
  return 1;
}
void bare(bool ready) {
  if (ready) return;
  print('work');
}
void barePair(bool ready) {
  if (ready) return;
  return;
}
String binding(Object value, String name) {
  if (value case String name) return name;
  return name;
}
