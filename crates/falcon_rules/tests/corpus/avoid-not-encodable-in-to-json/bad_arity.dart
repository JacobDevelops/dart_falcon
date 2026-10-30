class NeedsArgument {
  Object toJson(int value) => value;
}
class NeedsNamed {
  Object toJson({required int value}) => value;
}
class StaticOnly {
  static Object toJson() => 'value';
}
class Example {
  Object toJson() => {
    'argument': NeedsArgument(), /* expect: avoid-not-encodable-in-to-json */
    'named': NeedsNamed(), /* expect: avoid-not-encodable-in-to-json */
    'static': StaticOnly(), /* expect: avoid-not-encodable-in-to-json */
  };
}
