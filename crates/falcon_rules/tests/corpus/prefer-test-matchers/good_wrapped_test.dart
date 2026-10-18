import 'package:test/test.dart';
void checkObject(Object expected) { expect(1, expected); }
void checkNullableObject(Object? expected) { expect(1, expected); }
void checkFunction(bool Function(dynamic) expected) { expect(1, expected); }
void main() {
  test('wrappers preserve potentially dynamic matcher values', () {
    checkObject(equals(1));
    checkNullableObject(equals(1));
    checkFunction((value) => value == 1);
    expect(1, (dynamic value) => value == 1);
  });
}
