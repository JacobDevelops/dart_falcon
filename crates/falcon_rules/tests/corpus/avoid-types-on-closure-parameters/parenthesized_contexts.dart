void consume(void Function(int) callback) {}
void consumeNamed({required void Function(int) callback}) {}

void Function(int) synchronousReturn() {
  return ((int value) {});
}

void Function(int) arrowReturn() => ((int value) {}); /* expect: avoid-types-on-closure-parameters */

Future<void Function(int)> asynchronousReturn() async {
  return ((int value) {}); /* expect: avoid-types-on-closure-parameters */
}

void parenthesizedContexts(bool flag, Object subject) {
  final conditional = flag
      ? ((int first) {}) /* expect: avoid-types-on-closure-parameters */
      : ((int second) {}); /* expect: avoid-types-on-closure-parameters */
  void Function(int) selected = switch (subject) { _ => ((int value) {}) };
  final record = (((int value) {}),);
  final List<void Function(int)> list = [((int value) {})];
  final Map<int, void Function(int)> map = {0: ((int value) {})};
  consume(((int value) {}));
  consumeNamed(callback: ((int value) {})); /* expect: avoid-types-on-closure-parameters */
  void Function(int) assigned;
  assigned = ((int value) {}); /* expect: avoid-types-on-closure-parameters */
  conditional(0);
  selected(0);
  record.$1(0);
  list.first(0);
  map[0]!(0);
  assigned(0);
}
