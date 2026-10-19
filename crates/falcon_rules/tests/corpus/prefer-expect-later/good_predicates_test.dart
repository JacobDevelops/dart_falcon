import 'package:test/test.dart';
void main() {
  test('predicate internals do not determine actual type', () {
    expect(1, predicate((value) { final matcher = completes; return true; }));
    expect(1, predicate((value) { final matcher = emitsDone; return true; }));
    expect(1, predicate((value) { final matcher = throwsStateError; return true; }));
    expect(1, anyOf([completes, equals(1)]));
    expect(Stream.value(1), emits(1));
  });
}
