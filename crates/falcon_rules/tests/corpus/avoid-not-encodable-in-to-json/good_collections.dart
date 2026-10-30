extension type Field(String value) {}
class Plain {}
class Keyed<K, V> implements Iterable<V> {
  dynamic noSuchMethod(Invocation invocation) => throw 'not called';
}
class IntList implements List<int> {
  dynamic noSuchMethod(Invocation invocation) => throw 'not called';
}
class IntMap implements Map<String, int> {
  dynamic noSuchMethod(Invocation invocation) => throw 'not called';
}
class Example {
  Object toJson(Keyed<Plain, int> keyed, IntList list, IntMap map, Map<Field, int> fields) => {
    Field('key'): 1,
    'fields': fields,
    'spread': [...keyed],
    'list': list,
    'map': map,
    ...map,
    'nullable': [?1],
  };
}
