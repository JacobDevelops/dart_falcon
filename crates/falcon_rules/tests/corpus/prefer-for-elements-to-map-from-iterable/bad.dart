void bad() {
  var a = Map.fromIterable(list, key: (e) => e, value: (e) => e * 2); /* expect: prefer-for-elements-to-map-from-iterable */
  var b = Map.fromIterable(items, key: (k) => k.id, value: (v) => v.name); /* expect: prefer-for-elements-to-map-from-iterable */
  var c = Map.fromIterable(nums, key: (n) => n.toString(), value: (n) => n); /* expect: prefer-for-elements-to-map-from-iterable */
  var d = Map.fromIterable(data, value: (e) => e, key: (e) => e.hashCode); /* expect: prefer-for-elements-to-map-from-iterable */
  var e = Map.fromIterable(xs, key: (x) => x, value: (x) => x); /* expect: prefer-for-elements-to-map-from-iterable */
}
