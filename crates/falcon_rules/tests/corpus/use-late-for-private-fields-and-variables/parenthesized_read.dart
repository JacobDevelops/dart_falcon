// Analyzer-confirmed AdvB4 ul_paren_bang.dart: parentheses keep a nullable access.
int? _b;
void f() {
  print((_b)!);
}
