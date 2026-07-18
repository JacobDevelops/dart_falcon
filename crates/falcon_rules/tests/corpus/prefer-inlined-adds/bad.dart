void bad() {
  var a = [1]..add(2); /* expect: prefer-inlined-adds */
  var b = <int>[]..add(1); /* expect: prefer-inlined-adds */
  var c = []..add(1)..add(2); /* expect: prefer-inlined-adds */ /* expect: prefer-inlined-adds */
  var d = ['a']..add('b'); /* expect: prefer-inlined-adds */
  var e = <String>['x']..add('y'); /* expect: prefer-inlined-adds */
}
