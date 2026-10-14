import 'package:meta/meta.dart';

void f({required int b, int? a}) {}

void g(int positional, {required int b, required int c, int a = 0}) {}

void legacy({@required int? b, int? a}) {}

void onlyOptional({int? a, int? b}) {}

class Base {
  Base({required int b, int? a});
}

class Sub extends Base {
  // A lone optional super parameter does not count as an optional parameter.
  Sub({super.a, required super.b});
}

typedef Callback = void Function({required int b, int a});

void h(void Function({required int a, int? b}) cb) {}
