import 'package:test/test.dart';
void main() {
  test('awaited actual and expectation', () async {
    expect(await Future.value(1), 1);
    await expectLater(Future.value(1), completion(1));
    expect(() async => 1, throwsStateError);
    expect(Stream.value(1), emits(1));
    expect(1, equals(1));
  });
}
