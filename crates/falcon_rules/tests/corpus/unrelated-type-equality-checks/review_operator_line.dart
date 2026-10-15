class Left {}
class Right {}
Left identity(Left value) => value;
void multiline(Left left, Right right) {
  (identity(
    left,
  )) /* == in a comment */
      == /* expect: unrelated-type-equality-checks */
      right;
  left
      != /* expect: unrelated-type-equality-checks */
      right;
}
