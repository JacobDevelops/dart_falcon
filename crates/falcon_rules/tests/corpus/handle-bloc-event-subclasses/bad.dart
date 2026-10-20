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

abstract class GenericEvent {}
abstract class Value<T> extends GenericEvent {}
class IntValue extends Value<int> {}

class MismatchedValueBloc extends Bloc<GenericEvent, int> {
  MismatchedValueBloc() : super(0) { /* expect: handle-bloc-event-subclasses */
    on<Value<String>>((event, emit) {});
  }
}

class UnrelatedHelperBloc extends Bloc<Event, int> {
  UnrelatedHelperBloc.other(UnrelatedHelperBloc other) : super(0) { /* expect: handle-bloc-event-subclasses */
    other._registerHandlers();
  }
  UnrelatedHelperBloc.shadowed() : super(0) { /* expect: handle-bloc-event-subclasses */
    void _registerHandlers() {}
    _registerHandlers();
  }
  UnrelatedHelperBloc.staticCall() : super(0) { /* expect: handle-bloc-event-subclasses */
    _staticHelper();
  }
  UnrelatedHelperBloc.addEvent() : super(0) { /* expect: handle-bloc-event-subclasses */
    on<Start>((event, emit) {});
    add(Start());
  }
  UnrelatedHelperBloc.explicitAddEvent() : super(0) { /* expect: handle-bloc-event-subclasses */
    on<Start>((event, emit) {});
    this.add(Start());
  }
  void _registerHandlers() {
    on<Start>((event, emit) {});
    on<Stop>((event, emit) {});
  }
  static void _staticHelper() {}
}
