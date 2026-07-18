// Good: no `+` join of two string literals.
void examples() {
  const name = 'world';
  final a = 'foo' 'bar'; // adjacent string literals
  final b = 'hello ' + name; // right operand is not a literal
  final c = name + 'x'; // left operand is not a literal
  final d = 1 + 2; // not strings at all
  final e = 'count: ' + 3.toString(); // right operand is not a literal
  final f = ['a', 'b'].join(); // no `+` operator
  print([a, b, c, d, e, f]);
}
