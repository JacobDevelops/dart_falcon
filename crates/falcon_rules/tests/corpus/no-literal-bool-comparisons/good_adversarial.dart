// Analyzer-confirmed AdvB3: a parenthesized bool is not a BooleanLiteral.
void compare(bool b) {
  print(b == (true));
}
