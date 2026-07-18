void bad() {
  var a = items.where((e) => e is String); /* expect: prefer-iterable-where-type */
  var b = list.where((x) => x is int); /* expect: prefer-iterable-where-type */
  var c = things.where((t) => t is Widget); /* expect: prefer-iterable-where-type */
  var d = values.where((v) => v is double).toList(); /* expect: prefer-iterable-where-type */
  var e = data.where((item) => item is Map); /* expect: prefer-iterable-where-type */
}
