void bad(List<int> items) {
  var a = [1, 2]..addAll(items); /* expect: prefer-spread-collections */
  var b = <int>[]..addAll(items); /* expect: prefer-spread-collections */
  var e = []..addAll(items)..addAll(items); /* expect: prefer-spread-collections */ /* expect: prefer-spread-collections */
}
