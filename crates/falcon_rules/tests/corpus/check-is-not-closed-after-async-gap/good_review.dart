import 'package:bloc/bloc.dart';

class CounterBloc extends Bloc<int, int> {
  CounterBloc() : super(0) {
    on<int>((event, emit) async {
      await load();
      if (!emit.isDone) emit(1);
    });
  }
  Future<void> emitterReturn(int event, Emitter<int> emit) async {
    await load();
    if (emit.isDone) return;
    emit(2);
  }
  Future<void> equalsGuard() async {
    await load();
    if (isClosed == false) emit(3);
  }
  Future<void> negatedEqualsGuard() async {
    await load();
    if (!isClosed == true) emit(4);
  }
  Future<void> patternGuard() async {
    await load();
    if (isClosed case false) emit(5);
  }
  Future<void> negatedPatternGuard() async {
    await load();
    if (!isClosed case true) emit(5);
  }
  Future<void> whenGuard() async {
    final result = await load();
    if (result case final int value when !isClosed) {
      emit(value);
    }
  }
  Future<void> switchAwait(bool flag) async {
    final value = switch (flag) { true => await load(), false => 0 };
    emit(value);
  }
  Future<void> collectionAwait(bool flag) async {
    final values = [if (flag) await load()];
    emit(values.length);
  }
  Future<void> nullableAwait(Service? service) async {
    service?.consume(await load());
    emit(6);
  }
  Future<int> load() async => 1;
}
class Service { void consume(int value) {} }
