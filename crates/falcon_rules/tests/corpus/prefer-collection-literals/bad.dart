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

// A `{}` literal has static type `Map`, which IS assignable to a declared `Map`
// context — the concrete-type exception does not apply, so the diagnostic keeps
// firing. Only `LinkedHashMap`/`LinkedHashSet` contexts suppress.
void nonConcreteContext() {
  Map<int, int> m = LinkedHashMap(); /* expect: prefer-collection-literals */
  var n = LinkedHashMap(); /* expect: prefer-collection-literals */
}
