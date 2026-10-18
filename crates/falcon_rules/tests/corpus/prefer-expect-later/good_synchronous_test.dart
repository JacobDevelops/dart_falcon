import 'package:test/test.dart';
void main() {
  test('synchronous throw matchers', () {
    expect(() { throw StateError('bad'); }, throwsStateError);
    expect(() { throw 'bad'; }, throwsA(equals('bad')));
  });
}
