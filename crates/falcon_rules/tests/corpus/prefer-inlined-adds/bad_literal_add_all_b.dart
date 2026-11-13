void checks() {
  var a = ['a']..addAll(['b', 'c']); /* expect: prefer-inlined-adds */
  var b = <int>[]..addAll(<int>[1, 2]); /* expect: prefer-inlined-adds */
  var c = [1]..add(2)..addAll([3]); /* expect: prefer-inlined-adds */ /* expect: prefer-inlined-adds */
}
