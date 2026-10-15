void recordField((int, String) r) {
  r.$1.round(); /* expect: noop-primitive-operations */
}
void switchReceiver(bool b) {
  (switch (b) { true => 1, false => 2 }).round(); /* expect: noop-primitive-operations */
}
