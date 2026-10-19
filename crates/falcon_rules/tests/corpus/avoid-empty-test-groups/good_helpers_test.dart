import 'package:test/test.dart';
void registerTests() { test('registered indirectly', () {}); }
void main() {
  group('helper registration', () { registerTests(); });
  group('nested helper registration', () {
    group('inner', () { registerTests(); });
  });
  group('indirect group callback', () { group('delegated', registerTests); });
  group('callback tear-off', registerTests);
  final register = registerTests;
  group('local callable', () { register(); });
}
