void f(Object x) {
  if (!(x is int)) {} /* expect: prefer-is-not-operator */
  var a = !(x is String); /* expect: prefer-is-not-operator */
  final b = !(x is List<int>); /* expect: prefer-is-not-operator */
  final c = !(x is double); /* expect: prefer-is-not-operator */
  print(!(x is bool)); /* expect: prefer-is-not-operator */
  print('$a $b $c');
}
