// Ordinary production APIs are never constrained by test-description patterns.
void test(String description, void Function() callback) {}
void testWidgets(String description, void Function() callback) {}

void main() {
  test('unformatted helper', () {});
  testWidgets('unformatted widget helper', () {});
}
