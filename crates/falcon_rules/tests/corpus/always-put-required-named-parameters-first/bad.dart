import 'package:meta/meta.dart';

void f({int? a, required int b}) {} /* expect: always-put-required-named-parameters-first */

void g({int a = 0, required int b, required int c}) {} /* expect: always-put-required-named-parameters-first */ /* expect: always-put-required-named-parameters-first */

void legacy({int? a, @required int? b}) {} /* expect: always-put-required-named-parameters-first */

class A {
  final int x;
  final int y;

  A({this.x = 0, required this.y}); /* expect: always-put-required-named-parameters-first */

  void m({bool flag = false, required String name}) {} /* expect: always-put-required-named-parameters-first */
}

typedef Callback = void Function({int a, required int b}); /* expect: always-put-required-named-parameters-first */

void h() {
  void local({int? a, required int b}) {} /* expect: always-put-required-named-parameters-first */
  final literal = ({int? a, required int b}) => b; /* expect: always-put-required-named-parameters-first */
  local(b: 1);
  literal(b: 2);
}
