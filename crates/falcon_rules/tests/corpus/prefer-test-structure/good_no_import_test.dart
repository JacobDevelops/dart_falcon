void test(String description, dynamic callback) {}
void testWidgets(String description, dynamic callback) {}
void main() {
  test('unrelated function', () {});
  testWidgets('unrelated widget function', (tester) {});
}
