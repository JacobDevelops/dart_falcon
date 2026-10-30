import 'package:flutter_test/flutter_test.dart';
import 'package:test/test.dart' as framework;

void callback() {}

void main() {
  test('unit test', () {
    // arrange
    final a = 1;
    final b = 2;
    // act
    final c = a + 1;
    // assert
    expect(b, c);
    expect(c, 2);
  });
  framework.test('prefixed and parenthesized', (() {
    // Arrange
    final a = 1;
    // Act
    final b = a + 1;
    // Assert
    expect(b, 2);
  }));
  testWidgets('widget test', (tester) async {
    // arrange
    final a = 1;
    // act
    final b = a + 1;
    // assert
    expect(b, 2);
  });
  test('block markers', () {
    /* arrange */
    final a = 1;
    /* act */
    final b = a + 1;
    /* assert */
    expect(b, 2);
  });
  test('nested code', () {
    // arrange
    void helper() {
      // assert
      expect(1, 1);
    }
    // act
    helper();
    // assert
    expect(1, 1);
  });
  group('groups are not tests', () {
    setUp(() {});
    test('nested test', () {
      // arrange
      final a = 1;
      // act
      final b = a + 1;
      // assert
      expect(b, 2);
    });
  });
  test('tear-off', callback);
  final receiver = LocalApi();
  receiver.test('receiver method', () {});
}

class LocalApi {
  void test(String description, dynamic body) {}
  void method() { test('implicit receiver', () {}); }
}

void shadowed() {
  void test(String description, dynamic body) {}
  test('local function', () {});
}

void shadowedPrefix() {
  final framework = LocalApi();
  framework.test('local receiver', () {});
}
