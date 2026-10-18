import 'package:bloc/bloc.dart';

class Snapshot {
  int count = 0;
  Snapshot copyWith() => Snapshot();
}
class Counter extends Cubit<Snapshot> {
  Counter() : super(Snapshot());
  void unchanged() {
    emit(state); /* expect: emit-new-bloc-state-instances */
    emit(this.state); /* expect: emit-new-bloc-state-instances */
    this.emit(state); /* expect: emit-new-bloc-state-instances */
    super.emit((state)); /* expect: emit-new-bloc-state-instances */
    emit(state..count = 1); /* expect: emit-new-bloc-state-instances */
    emit((this.state)..count = 2); /* expect: emit-new-bloc-state-instances */
  }
}
class Event {}
class CounterBloc extends Bloc<Event, Snapshot> {
  CounterBloc() : super(Snapshot()) {
    on<Event>((event, emit) {
      emit(state); /* expect: emit-new-bloc-state-instances */
      emit(this.state..count = 3); /* expect: emit-new-bloc-state-instances */
    });
  }
}
