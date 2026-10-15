// Analyzer-confirmed AdvB3 inferred and non-local boolean operands.
bool top = true;

void compare() {
  print(top == true); /* expect: no-literal-bool-comparisons */
}


void genericMember(List<bool> l) {
  print(l.first == true); /* expect: no-literal-bool-comparisons */
}

