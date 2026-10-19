import 'package:test/test.dart';
void _aTests() { test('works', () {}); }
void _bTests() { test('works', () {}); }
void main() {
  group('A', _aTests);
  group('B', _bTests);
  for (final item in [1, 2]) { test('item $item', () {}); }
}
