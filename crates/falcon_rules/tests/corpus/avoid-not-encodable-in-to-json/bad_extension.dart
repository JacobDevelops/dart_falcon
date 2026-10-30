extension type NumberKey(int value) {}
extension type Wrapped<T>(T value) {}
class Example {
  Object toJson(Map<NumberKey, int> values, Wrapped<Set<int>> wrapped) => {
    NumberKey(1): 'invalid', /* expect: avoid-not-encodable-in-to-json */
    'map': values, /* expect: avoid-not-encodable-in-to-json */
    'wrapped': wrapped, /* expect: avoid-not-encodable-in-to-json */
  };
}
