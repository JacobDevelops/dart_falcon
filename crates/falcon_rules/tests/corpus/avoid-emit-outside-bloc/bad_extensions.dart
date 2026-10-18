import 'package:bloc/bloc.dart';

class Counter extends Cubit<int> {
  Counter() : super(0);
}
class SafeCounter extends Counter {
  @override
  void emit(int value) { super.emit(value); }
}
extension Outside on Counter {
  void update(Counter other) {
    emit(1); /* expect: avoid-emit-outside-bloc */
    this.emit(2); /* expect: avoid-emit-outside-bloc */
    other.emit(3); /* expect: avoid-emit-outside-bloc */
    this..emit(4); /* expect: avoid-emit-outside-bloc */
  }
}
void overridden(SafeCounter counter) {
  counter.emit(5); /* expect: avoid-emit-outside-bloc */
}
