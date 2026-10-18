import 'package:bloc/bloc.dart';

sealed class Event {}
final class Start extends Event {}
final class Stop extends Event {}

class CounterBloc extends Bloc<Event, int> {
  CounterBloc() : super(0) { /* expect: handle-bloc-event-subclasses */
    on<Start>((event, emit) {});
  }
  CounterBloc.empty() : super(0) {} /* expect: handle-bloc-event-subclasses */
  CounterBloc.partialCascade() : super(0) { /* expect: handle-bloc-event-subclasses */
    this..on<Start>((event, emit) {});
  }
  CounterBloc.otherReceiver(Bloc<Event, int> other) : super(0) { /* expect: handle-bloc-event-subclasses */
    other
      ..on<Start>((event, emit) {})
      ..on<Stop>((event, emit) {});
  }
  CounterBloc.localOn() : super(0) { /* expect: handle-bloc-event-subclasses */
    void on(Object handler) {}
    on((Event event, Emitter<int> emit) {});
  }
}
