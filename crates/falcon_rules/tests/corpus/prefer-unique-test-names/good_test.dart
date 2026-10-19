import 'package:flutter_test/flutter_test.dart' as t;
void main() {
  t.test('one', () {});
  t.test('two', () {});
  t.group('first', () { t.test('same', () {}); });
  t.group('second', () { t.test('same', () {}); });
  t.testWidgets('widget', (tester) {});
  for (var i = 0; i < 2; i++) { t.test('case $i', () {}); }
  void test(String name, dynamic callback) {}
  test('one', () {});
}
