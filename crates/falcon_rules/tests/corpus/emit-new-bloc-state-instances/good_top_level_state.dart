import 'package:bloc/bloc.dart';

final state = Object();
class Counter extends Cubit<Object> {
  Counter() : super(Object());
  void update() { emit(state); }
}
