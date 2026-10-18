import 'package:bloc/bloc.dart';

void emit(Object value) {}
class Counter extends Cubit<Object> {
  Counter() : super(Object());
  void update() { emit(state); }
}
