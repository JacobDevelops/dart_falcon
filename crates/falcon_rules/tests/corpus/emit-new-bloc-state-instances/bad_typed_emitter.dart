import 'package:bloc/bloc.dart';

class Event {}
class CounterBloc extends Bloc<Event, int> {
  CounterBloc() : super(0) { on<Event>(_handle); }
  void _handle(Event event, Emitter<int> emit) {
    emit(state); /* expect: emit-new-bloc-state-instances */
  }
}
