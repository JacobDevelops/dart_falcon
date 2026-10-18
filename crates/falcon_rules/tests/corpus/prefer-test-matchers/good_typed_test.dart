import 'package:test/test.dart';
class CustomMatcher implements Matcher {}
void main() {
  test('matcher subclasses and unresolved values', () {
    Matcher matcher = CustomMatcher();
    expect(1, matcher);
    expect(1, CustomMatcher());
    expect(1, equals(1));
    expect(1, isA<int>());
    expect(1, predicate((value) => true));
  });
}
