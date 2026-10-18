import 'package:bloc/bloc.dart';

class Snapshot {
  int count = 0;
  Snapshot copyWith() => Snapshot();
}
class Counter extends Cubit<Snapshot> {
  Counter() : super(Snapshot());
  void changed() {
    emit(Snapshot());
    emit(state.copyWith());
    emit(Snapshot()..count++);
  }
  void shadowedState(Snapshot state) { emit(state); }
  void shadowedEmit(void Function(Snapshot) emit) { emit(state); }
  void nested() {
    void emit(Snapshot value) {}
    emit(this.state);
  }
}
class Event {}
class CounterBloc extends Bloc<Event, Snapshot> {
  CounterBloc() : super(Snapshot()) {
    on<Event>((event, emit) {
      emit(state.copyWith());
      void nested(void Function(Snapshot) emit) { emit(state); }
    });
  }
}
class Unrelated {
  final state = Snapshot();
  void emit(Snapshot value) {}
  void run() { emit(state); }
}
