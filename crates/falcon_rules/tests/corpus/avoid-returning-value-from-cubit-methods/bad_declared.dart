import 'package:bloc/bloc.dart';

class Counter extends Cubit<int> {
  Counter() : super(0);
  int count() => state; /* expect: avoid-returning-value-from-cubit-methods, msg: "Avoid returning values from `Cubit` methods; expose results through state instead." */
  void increment() {}
}
