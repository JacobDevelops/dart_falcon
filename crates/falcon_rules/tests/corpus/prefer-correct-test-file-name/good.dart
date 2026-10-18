// A similarly named local API without a test-framework import is not a test.
void test(String name, dynamic body) {}
void group(String name, dynamic body) {}
void expect(dynamic actual, dynamic expected) {}
void main() { group('empty', () {}); test('same', () {}); test('same', () {}); expect(Future.value(1), 1); }
