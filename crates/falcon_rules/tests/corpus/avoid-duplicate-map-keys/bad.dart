void f(bool condition) {
  final first = {'a': 1, 'a': 2}; /* expect: avoid-duplicate-map-keys */
  final second = {1: 'a', 1: 'b'}; /* expect: avoid-duplicate-map-keys */
  final third = {true: 1, true: 2}; /* expect: avoid-duplicate-map-keys */
  final fourth = {null: 1, null: 2}; /* expect: avoid-duplicate-map-keys */
  final fifth = {'a': 1, ...{'a': 2}}; /* expect: avoid-duplicate-map-keys */
  final sixth = {'a': 1, if (condition) 'a': 2}; /* expect: avoid-duplicate-map-keys */
  final seventh = {if (condition) 'a': 1, 'a': 2}; /* expect: avoid-duplicate-map-keys */
}
