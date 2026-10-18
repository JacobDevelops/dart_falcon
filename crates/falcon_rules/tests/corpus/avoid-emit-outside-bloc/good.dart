import 'package:bloc/bloc.dart';

class Counter extends Cubit<int> {
  Counter() : super(0);
  void increment() { emit(state + 1); }
  void explicit() { this.emit(1); }
  void inherited() { super.emit(2); }
  void cascade() { this..emit(3); }
  void internal(Counter other) { other.emit(4); }
}
class Other {
  void emit(int value) {}
}
void unrelated(Other other, dynamic unknown, void Function(int) emit) {
  other.emit(1);
  unknown.emit(2);
  emit(3);
  other..emit(4);
}
void publicApi(Counter counter) { counter.increment(); }
