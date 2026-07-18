// Bad: calls to the top-level print function.
void main() {
  print('hello'); /* expect: avoid-print */
  print(42); /* expect: avoid-print */
  const value = 7;
  print(value); /* expect: avoid-print */
  print('sum: ${value + 1}'); /* expect: avoid-print */
  final list = [1, 2, 3];
  list.forEach((e) => print(e)); /* expect: avoid-print */
  print('done'); /* expect: avoid-print */
}
