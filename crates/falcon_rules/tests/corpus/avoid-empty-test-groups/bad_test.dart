import 'package:test/test.dart';
void main() {
  group('empty', () {}); /* expect: avoid-empty-test-groups */
  group('setup only', () { setUp(() {}); }); /* expect: avoid-empty-test-groups */
  group('nested empty', () {
    group('also empty', () {}); /* expect: avoid-empty-test-groups */
  });
  group('statements', () { final value = 1; final other = value + 1; }); /* expect: avoid-empty-test-groups */
  group('lifecycle callback calls do not register tests', () { setUp(() { print('setup'); }); }); /* expect: avoid-empty-test-groups */
}
