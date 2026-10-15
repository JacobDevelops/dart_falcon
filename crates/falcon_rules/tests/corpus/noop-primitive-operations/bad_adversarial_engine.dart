// Covers record fields, String indexing and multiplication, and switch joins.
void recordField((int, String) r) {
  r.$1.round(); /* expect: noop-primitive-operations */
}
void indexed(String s) {
  s[0].toString(); /* expect: noop-primitive-operations */
}
void multiply(String s) {
  (s * 2).toString(); /* expect: noop-primitive-operations */
}
void switchReceiver(bool b) {
  (switch (b) { true => 1, false => 2 }).round(); /* expect: noop-primitive-operations */
}
