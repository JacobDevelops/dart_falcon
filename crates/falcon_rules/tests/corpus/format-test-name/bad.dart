// Test-like helpers outside test-runner files are not test registrations.
import 'package:test/test.dart';

void main() {
  test('unformatted helper', () {});
  group('unformatted group', () {});
}
