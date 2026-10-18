import 'package:flutter_test/flutter_test.dart' as t;
void callback() {}
void main() {
  t.group('one', () { t.test('works', () {}); });
  t.group('nested', () { t.group('inner', () { t.test('works', () {}); }); });
  t.group('widget', () { t.testWidgets('works', (tester) {}); });
  t.group('external callback', callback);
  t.group('skipped tests still registered', () { t.test('skipped', () {}, skip: true); });
  void group(String description, dynamic body) {}
  group('local function', () {});
}
