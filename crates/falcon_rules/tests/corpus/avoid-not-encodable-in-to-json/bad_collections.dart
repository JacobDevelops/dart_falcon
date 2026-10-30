class Plain {}
class BadList implements List<Plain> {
  dynamic noSuchMethod(Invocation invocation) => throw 'not called';
}
class Example {
  Object toJson(Iterable<int> iterable, Uri uri, BigInt big, BadList values) => {
    'iterable': iterable, /* expect: avoid-not-encodable-in-to-json */
    'uri': uri, /* expect: avoid-not-encodable-in-to-json */
    'big': big, /* expect: avoid-not-encodable-in-to-json */
    'list': values, /* expect: avoid-not-encodable-in-to-json */
    'spread': [...values], /* expect: avoid-not-encodable-in-to-json */
    'record': (1, 2), /* expect: avoid-not-encodable-in-to-json */
    ...{'entry': Plain()}, /* expect: avoid-not-encodable-in-to-json */
    'nullable': [?Plain()], /* expect: avoid-not-encodable-in-to-json */
  };
}
