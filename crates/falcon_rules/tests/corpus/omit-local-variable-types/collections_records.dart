List<T> emptyElements<T>() => [];
Map<K, V> emptyEntries<K, V>() => {};
abstract class CollectionSource<A, B> implements Iterable<B> {}

void collectionsAndRecords(Iterable<int> source, Object? candidate) {
  List<int> list = [for (var item in source) item]; /* expect: omit-local-variable-types */
  Set<int> set = {for (var item in source) item}; /* expect: omit-local-variable-types */
  Map<int, int> map = {for (var item in source) item: item}; /* expect: omit-local-variable-types */
  List<int> counted = [for (var index = 0; index < 2; index++) index]; /* expect: omit-local-variable-types */
  List<int> promoted = [if (candidate is int) candidate]; /* expect: omit-local-variable-types */
  Map<int, int> promotedMap = {if (candidate is int) candidate: candidate}; /* expect: omit-local-variable-types */
  List<int> spread = [...emptyElements()]; /* expect: omit-local-variable-types */
  Map<int, String> spreadMap = {...emptyEntries()}; /* expect: omit-local-variable-types */
  (List<int>, {List<String> names}) record = (emptyElements(), names: emptyElements()); /* expect: omit-local-variable-types */
  final inferred = (1, name: 'value');
  int positional = inferred.$1; /* expect: omit-local-variable-types */
  String named = inferred.name; /* expect: omit-local-variable-types */
  print([list, set, map, counted, promoted, promotedMap, spread, spreadMap, record, positional, named]);
}

void uncontextualizedCollections(CollectionSource<String, int> source) {
  final inferredList = [for (var item in source) item];
  final inferredSet = {for (var item in source) item};
  final inferredMap = {for (var item in source) item: item};
  List<int> list = inferredList; /* expect: omit-local-variable-types */
  Set<int> set = inferredSet; /* expect: omit-local-variable-types */
  Map<int, int> map = inferredMap; /* expect: omit-local-variable-types */
  print([list, set, map]);
}
