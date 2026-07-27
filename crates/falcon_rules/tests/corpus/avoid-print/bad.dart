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

// A local shadow declared in a try, catch or finally body is scoped to that
// body and must not suppress the call after the statement.
void scopedShadow() {
  try {
    void print(Object? value) {}
    print('shadowed');
  } catch (_) {
    void print(Object? value) {}
    print('shadowed');
  } finally {
    void print(Object? value) {}
    print('shadowed');
  }
  print('real'); /* expect: avoid-print */
}
