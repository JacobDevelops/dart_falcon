import 'package:bloc/bloc.dart';

class Counter extends Cubit<int> { Counter() : super(0); }
extension SeparateOperation on Counter {
  void emit(String message) {}
  void update() {
    this.emit(1); /* expect: avoid-emit-outside-bloc */
  }
}
