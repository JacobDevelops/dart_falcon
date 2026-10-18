import 'package:bloc/bloc.dart';

class CounterCubit extends Cubit<int> {
  CounterCubit() : super(0);
  Future<void> unguarded() async {
    await load();
    emit(1); /* expect: check-is-not-closed-after-async-gap */
  }
  Future<void> explicitReceiver() async {
    await load();
    this.emit(2); /* expect: check-is-not-closed-after-async-gap */
  }
  Future<void> staleCheck() async {
    if (!isClosed) {
      await load();
      emit(3); /* expect: check-is-not-closed-after-async-gap */
    }
  }
  Future<void> wrongGuard(CounterCubit other) async {
    await load();
    if (!other.isClosed) {
      emit(4); /* expect: check-is-not-closed-after-async-gap */
    }
  }
  Future<void> staleReturn() async {
    if (isClosed) return;
    await load();
    emit(5); /* expect: check-is-not-closed-after-async-gap */
  }
  Future<void> awaitedArgument() async {
    emit(await value()); /* expect: check-is-not-closed-after-async-gap */
  }
  Future<void> guardedAwaitedArgument() async {
    if (!isClosed) emit(await value()); /* expect: check-is-not-closed-after-async-gap */
  }
  Future<void> gapBeforeConditionalArgument(bool ready) async {
    await load();
    emit(ready ? await value() : 0); /* expect: check-is-not-closed-after-async-gap */
  }
  Future<int> value() async => 6;
  Future<void> load() async {}
}

class CounterBloc extends Bloc<int, int> {
  CounterBloc() : super(0) {
    on<int>((event, emit) async {
      await load();
      add(1); /* expect: check-is-not-closed-after-async-gap */
      emit(2); /* expect: check-is-not-closed-after-async-gap */
    });
  }
  Future<void> handle(int event, Emitter<int> emit) async {
    await load();
    emit(3); /* expect: check-is-not-closed-after-async-gap */
  }
  Future<void> shadowedGuard(bool isClosed) async {
    await load();
    if (!isClosed) {
      add(4); /* expect: check-is-not-closed-after-async-gap */
    }
  }
  Future<void> load() async {}
}
