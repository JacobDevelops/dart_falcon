import 'package:test/test.dart';
void testCase(String name, dynamic body) { test(name, body); }
void main() {
  group('custom registration', () { testCase('case', () {}); });
  group('nested registration', () { group('inner', () { test('case', () {}); }); });
}
