bool direct(bool value) {
  if (value) return true;
  return false; /* expect: prefer-returning-condition */
}
bool inverted(bool value) {
  if (value) { return false; }
  return true; /* expect: prefer-returning-condition */
}
bool parenthesized(bool value) {
  if ((value)) { return (true); }
  return (false); /* expect: prefer-returning-condition */
}
class Worker {
  bool check(int value) {
    if (value > 0) { return true; }
    return false; /* expect: prefer-returning-condition */
  }
}
void closure() {
  final check = (bool value) {
    if (value) return true;
    return false; /* expect: prefer-returning-condition */
  };
  print(check(true));
}
bool nested(bool value, bool other) {
  if (other) {
    if (value) return false;
    return true; /* expect: prefer-returning-condition */
  }
  return false;
}
bool nestedBlocks(bool value) {
  if (value) {{ return true; }}
  return false; /* expect: prefer-returning-condition */
}
