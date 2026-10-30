import 'dart:async';
import 'package:bloc/bloc.dart';

class Counter extends Cubit<int> {
  Counter() : super(0);
  int count() => state; /* expect: avoid-returning-value-from-cubit-methods, msg: "Avoid returning values from `Cubit` methods; expose results through state instead." */
  String label() => 'counter'; /* expect: avoid-returning-value-from-cubit-methods */
  Future<int> load() async => 1; /* expect: avoid-returning-value-from-cubit-methods */
  dynamic result() => state; /* expect: avoid-returning-value-from-cubit-methods */
  inferred() => 1; /* expect: avoid-returning-value-from-cubit-methods */
  inferredBlock() { return 'value'; } /* expect: avoid-returning-value-from-cubit-methods */
  FutureOr<int> later() => 1; /* expect: avoid-returning-value-from-cubit-methods */
  values() sync* { yield 1; } /* expect: avoid-returning-value-from-cubit-methods */
  events() async* { yield 1; } /* expect: avoid-returning-value-from-cubit-methods */
}
class Child extends Counter {
  int another() => 1; /* expect: avoid-returning-value-from-cubit-methods */
  int count() => 2;
}
