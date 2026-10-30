import 'package:flutter_test/flutter_test.dart';
import 'package:test/test.dart' as tests;

const action = 'work';
String runtimeName() => 'unformatted';
class Helpers {
  void test(String name, dynamic callback) {}
  void testWidgets(String name, dynamic callback) {}
}
void main() {
  test('should work.', () {});
  tests.test('should also work.', () {});
  test('should $action.', () {});
  test(r'should preserve $literal.', () {});
  testWidgets('renders correctly.', (tester) {});
  group('any group description', () {});
  test(runtimeName(), () {});
  final helpers = Helpers();
  helpers.test('unformatted', () {});
  helpers.testWidgets('unformatted', () {});
  {
    void test(String name, dynamic callback) {}
    test('shadowed', () {});
  }
}
