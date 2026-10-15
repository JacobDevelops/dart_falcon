// Analyzer-confirmed AdvB3: parentheses preserve a separate AST expression.
void parenthesizedPrint(Object o) {
  print((o.toString()));
}
