import 'package:flutter_test/flutter_test.dart' as framework;
void main() {
  framework.test('unrestricted', () {});
  framework.testWidgets('broken', (tester) {}); /* expect: format-test-name */
  framework.group('unrestricted', () {});
}
