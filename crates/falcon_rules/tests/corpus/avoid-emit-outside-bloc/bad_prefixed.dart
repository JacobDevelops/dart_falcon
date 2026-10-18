import 'package:bloc/bloc.dart' as b;

class Counter extends b.Cubit<int> {
  Counter() : super(0);
}
void update(Counter counter) {
  counter.emit(1); /* expect: avoid-emit-outside-bloc */
}
