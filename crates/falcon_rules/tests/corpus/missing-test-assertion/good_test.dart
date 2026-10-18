import 'package:test/test.dart';
import 'package:test/test.dart' as t;
import 'package:flutter_test/flutter_test.dart' as f;
import 'package:mocktail/mocktail.dart' as mock;
void checkResult() {}
void main() {
  test('expect', () { expect(1, 1); });
  t.test('prefixed', () => t.expect(1, 1));
  test('later', () async { await expectLater(1, 1); });
  test('async wrapper', () { expectAsync0(() {}); });
  test('async argument wrapper', () { expectAsync1((value) {}); });
  test('async until', () { expectAsyncUntil1((value) {}, () => true); });
  test('fail', () { fail('unreachable'); });
  test('verify', () { mock.verify(() {}); });
  test('verify never', () { mock.verifyNever(() {}); });
  test('verify order', () { mock.verifyInOrder([() {}]); });
  test('configured helper', () { checkResult(); });
  f.testWidgets('widget', (tester) async { await tester.pump(); f.expect(1, 1); });
}
class OtherSuite { void test(String name, Function body) {} void run() { test('not a package test', () {}); } }
void localSuite() { void test(String name, Function body) {} test('local', () {}); }

void runCallback(void Function() callback) => callback();
void nestedAssertions() {
  test('passed callback', () { runCallback(() { expect(1, 1); }); });
  test('invoked closure', () { (() { expect(1, 1); })(); });
  test('called local function', () { void check() { expect(1, 1); } check(); });
  test('passed local closure', () { final check = () { expect(1, 1); }; runCallback(check); });
  test('collection callback', () { [1].forEach((value) { expect(value, 1); }); });
}

void interactionAssertions(Object instance) {
  test('no more interactions', () { mock.verifyNoMoreInteractions(instance); });
  test('zero interactions', () { mock.verifyZeroInteractions(instance); });
}
