import 'package:bloc/bloc.dart';

class CounterBloc extends Bloc<int, int> {
  CounterBloc() : super(0);
  Future<void> wrongEmitter(Emitter<int> emit, Emitter<int> other) async {
    await load();
    if (!other.isDone) {
      emit(1); /* expect: check-is-not-closed-after-async-gap */
    }
  }
  Future<void> staleEmitter(Emitter<int> emit) async {
    if (!emit.isDone) {
      await load();
      emit(2); /* expect: check-is-not-closed-after-async-gap */
    }
  }
  Future<void> load() async {}
}
