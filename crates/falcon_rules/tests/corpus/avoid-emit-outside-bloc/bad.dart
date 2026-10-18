import 'package:flutter_bloc/flutter_bloc.dart';

class Counter extends Cubit<int> {
  Counter() : super(0);
}
class Event {}
class CounterBloc extends Bloc<Event, int> {
  CounterBloc() : super(0);
}
void update(Counter counter, Counter? optional, CounterBloc bloc) {
  counter.emit(1); /* expect: avoid-emit-outside-bloc */
  optional?.emit(2); /* expect: avoid-emit-outside-bloc */
  bloc.emit(3); /* expect: avoid-emit-outside-bloc */
  counter..emit(4); /* expect: avoid-emit-outside-bloc */
  counter..emit(5)..emit(6); /* expect: avoid-emit-outside-bloc */ /* expect: avoid-emit-outside-bloc */
}
class Presenter {
  final Counter counter;
  Presenter(this.counter);
  void refresh() { counter.emit(7); } /* expect: avoid-emit-outside-bloc */
}
