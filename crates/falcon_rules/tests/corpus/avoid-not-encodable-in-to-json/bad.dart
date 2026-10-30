class Plain { const Plain(); }
class Example {
  Object toJson(Set<String> values, DateTime date, Duration delay) => {
    'set': values, /* expect: avoid-not-encodable-in-to-json */
    'date': date, /* expect: avoid-not-encodable-in-to-json */
    'duration': delay, /* expect: avoid-not-encodable-in-to-json */
    'callback': () => null, /* expect: avoid-not-encodable-in-to-json */
    'plain': Plain(), /* expect: avoid-not-encodable-in-to-json */
    'new': new Plain(), /* expect: avoid-not-encodable-in-to-json */
    'const': const Plain(), /* expect: avoid-not-encodable-in-to-json */
    'nested': [
      Plain(), /* expect: avoid-not-encodable-in-to-json */
      if (true) Plain(), /* expect: avoid-not-encodable-in-to-json */
      for (final item in values) Plain(), /* expect: avoid-not-encodable-in-to-json */
    ],
    42: 'key', /* expect: avoid-not-encodable-in-to-json */
  };
}
class Direct {
  Object toJson() { return Plain(); } /* expect: avoid-not-encodable-in-to-json */
}
class Custom {
  Object customToJson() => Plain(); /* expect: avoid-not-encodable-in-to-json */
}
class Typed {
  Object toJson(List<Plain> values) => values; /* expect: avoid-not-encodable-in-to-json */
}
class Spreads {
  Object toJson(List<Plain> values) => [
    ...values, /* expect: avoid-not-encodable-in-to-json */
  ];
}
class Conditional {
  Object toJson(bool condition) => condition
      ? Plain() /* expect: avoid-not-encodable-in-to-json */
      : Plain(); /* expect: avoid-not-encodable-in-to-json */
}
