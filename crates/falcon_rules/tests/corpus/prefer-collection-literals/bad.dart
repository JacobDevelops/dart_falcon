import 'dart:collection';

void bad() {
  var a = List(); /* expect: prefer-collection-literals */
  var b = Map(); /* expect: prefer-collection-literals */
  var c = Set(); /* expect: prefer-collection-literals */
  var d = LinkedHashMap(); /* expect: prefer-collection-literals */
  var e = LinkedHashSet(); /* expect: prefer-collection-literals */
  var f = List<int>(); /* expect: prefer-collection-literals */
  var g = new Set(); /* expect: prefer-collection-literals */
  var h = new Map<String, int>(); /* expect: prefer-collection-literals */
}
