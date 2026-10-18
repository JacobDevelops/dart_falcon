bool direct(bool value) { return value; }
bool inverted(bool value) { return !value; }
bool extraWork(bool value) {
  if (value) { print('work'); return true; }
  return false;
}
bool same(bool value) {
  if (value) return true;
  return true;
}
bool intervening(bool value) {
  if (value) return true;
  print('other');
  return false;
}
bool withElse(bool value) {
  if (value) return true;
  else return false;
}
bool pattern(Object value) {
  if (value case int number when number > 0) return true;
  return false;
}
int nonBoolean(bool value) {
  if (value) return 1;
  return 0;
}
bool arbitrary(bool value, bool result) {
  if (value) return result;
  return false;
}
bool separateBlock(bool value) {
  if (value) return true;
  { return false; }
}
