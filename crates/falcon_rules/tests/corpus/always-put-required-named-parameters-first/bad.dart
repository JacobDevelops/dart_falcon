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

class Base {
  Base({required int b, int? a});
}

class Sub extends Base {
  // Optional super parameters count as optional parameters.
  Sub({super.a, required super.b}); /* expect: always-put-required-named-parameters-first */
}

class Widgetish extends Base {
  final int child;
  Widgetish({super.a, required this.child}) : super(b: 0); /* expect: always-put-required-named-parameters-first */
}
