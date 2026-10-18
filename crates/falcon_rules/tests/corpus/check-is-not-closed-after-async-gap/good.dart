import 'package:bloc/bloc.dart';

class CounterCubit extends Cubit<int> {
  CounterCubit() : super(0);
  void synchronous() { emit(0); }
  Future<void> branchGuard() async {
    await load();
    if (!isClosed) { emit(1); }
  }
  Future<void> conjunctionGuard(bool ready) async {
    await load();
    if (!isClosed && ready) { emit(1); }
  }
  Future<void> disjunctionReturn(bool paused) async {
    await load();
    if (isClosed || paused) return;
    emit(1);
  }
  Future<void> returnGuard() async {
    await load();
    if (isClosed) return;
    emit(2);
  }
  Future<void> elseGuard() async {
    await load();
    if (this.isClosed) { print('closed'); } else { this.emit(3); }
  }
  Future<void> throwGuard() async {
    await load();
    if (isClosed) { throw StateError('closed'); }
    emit(4);
  }
  Future<void> freshGuard() async {
    await load();
    if (isClosed) return;
    await load();
    if (isClosed) return;
    emit(5);
  }
  Future<void> nestedIsolation() async {
    await load();
    final callback = () { emit(6); };
    print(callback);
  }
  Future<void> nestedAwaitIsolation() async {
    final callback = () async { await load(); };
    print(callback);
    emit(7);
  }
  void nestedAwaitArgument() {
    emit((() async { await load(); return 7; }).hashCode);
  }
  Future<void> conditionalAwaitArgument(bool ready) async {
    emit(ready ? await value() : 0);
  }
  Future<int> value() async => 7;
  Future<void> unrelatedReceiver(CounterCubit other) async {
    await load();
    other.emit(8);
  }
  Future<void> localShadow(void Function(int) emit) async {
    await load();
    emit(9);
  }
  Future<void> shortCircuit() async {
    await load();
    !isClosed && send();
  }
  bool send() { emit(10); return true; }
  Future<void> load() async {}
}

class Service {
  Future<void> run() async {
    await load();
    emit(1);
  }
  void emit(int value) {}
  Future<void> load() async {}
}

class CustomCubit extends Cubit<int> {
  CustomCubit() : super(0);
  void emit(int value) {}
}

class ChildCustomCubit extends CustomCubit {
  Future<void> inheritedOverride() async {
    await Future<void>.value();
    emit(1);
  }
}
