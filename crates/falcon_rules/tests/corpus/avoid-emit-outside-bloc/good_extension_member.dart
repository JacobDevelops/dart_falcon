import 'package:bloc/bloc.dart';

class Counter extends Cubit<int> { Counter() : super(0); }
extension SeparateOperation on Counter {
  void emit(String message) {}
  void update() {
    emit('local extension method');
  }
}
