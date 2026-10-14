import 'package:flutter_test/flutter_test.dart';

void main() {
  testWidgets('flutter_test re-exports fail', (tester) async {
    try {
      await tester.pump();
      fail('Expected an error'); /* expect: use-test-throws-matchers */
    } catch (error) {
      expect(error, isA<StateError>());
    }
  });
}
