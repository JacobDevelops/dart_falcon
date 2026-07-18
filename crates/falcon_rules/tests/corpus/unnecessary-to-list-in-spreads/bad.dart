void bad() {
  var a = [...items.toList()]; /* expect: unnecessary-to-list-in-spreads */
  var b = [1, ...more.toList()]; /* expect: unnecessary-to-list-in-spreads */
  var c = <int>{...values.toList()}; /* expect: unnecessary-to-list-in-spreads */
  var d = [...?maybe.toList()]; /* expect: unnecessary-to-list-in-spreads */
  var e = [...first.toList(), ...second.toList()]; /* expect: unnecessary-to-list-in-spreads */ /* expect: unnecessary-to-list-in-spreads */
}
