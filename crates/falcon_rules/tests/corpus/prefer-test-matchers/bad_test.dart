import 'package:test/test.dart';
void main() {
  test('manual expectations', () async {
    expect(true, true); /* expect: prefer-test-matchers */
    expect(false, false); /* expect: prefer-test-matchers */
    expect(null, null); /* expect: prefer-test-matchers */
    expect(1 == 1, true); /* expect: prefer-test-matchers */
    await expectLater(false, false); /* expect: prefer-test-matchers */
  });
}
