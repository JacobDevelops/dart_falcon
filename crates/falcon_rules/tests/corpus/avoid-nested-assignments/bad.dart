void f(dynamic x, dynamic y, dynamic z) {
  print(x = 1); /* expect: avoid-nested-assignments */
  final value = (x = 2); /* expect: avoid-nested-assignments */
  returnValue(x += 3); /* expect: avoid-nested-assignments */
  final message = '${x = 4}'; /* expect: avoid-nested-assignments */
  print(z ??= 6); /* expect: avoid-nested-assignments */
}
