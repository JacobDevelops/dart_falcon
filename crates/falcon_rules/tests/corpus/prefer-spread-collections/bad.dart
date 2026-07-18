void bad() {
  var a = [1, 2]..addAll([3, 4]); /* expect: prefer-spread-collections */
  var b = <int>[]..addAll([5]); /* expect: prefer-spread-collections */
  var c = {1, 2}..addAll({3}); /* expect: prefer-spread-collections */
  var d = <String>{}..addAll({'a'}); /* expect: prefer-spread-collections */
  var e = []..addAll([1])..addAll([2]); /* expect: prefer-spread-collections */ /* expect: prefer-spread-collections */
}
