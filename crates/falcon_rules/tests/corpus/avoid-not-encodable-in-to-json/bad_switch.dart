class NotSerializable {}
class Example {
  Object toJson(int value) => switch (value) {
    0 => NotSerializable(), /* expect: avoid-not-encodable-in-to-json */
    _ => {'value': NotSerializable()}, /* expect: avoid-not-encodable-in-to-json */
  };
}
