import 'package:test/test.dart';
import 'package:test/test.dart' as t;
import 'package:flutter_test/flutter_test.dart' as f;
void main() {
  test('empty', () {}); /* expect: missing-test-assertion */
  test('setup only', () { final value = 1; print(value); }); /* expect: missing-test-assertion */
  t.test('arrow', () => print('nothing asserted')); /* expect: missing-test-assertion */
  test('unused closure', () { final assertion = () { expect(1, 1); }; }); /* expect: missing-test-assertion */
  test('local shadow', () { void expect(Object? a, Object? b) {} expect(1, 1); }); /* expect: missing-test-assertion */
  test('unused local function', () { void assertion() { expect(1, 1); } }); /* expect: missing-test-assertion */
  f.testWidgets('widget without expectation', (tester) async { await tester.pump(); }); /* expect: missing-test-assertion */
}
class Checker { void expect(Object? a, Object? b) {} }
void objectAssertion() {
  test('object method is not an assertion', () { Checker().expect(1, 1); }); /* expect: missing-test-assertion */
}
