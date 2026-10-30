import 'dart:async';
import 'package:bloc/bloc.dart';

class Counter extends Cubit<int> {
  Counter() : super(0);
  void increment() => emit(state + 1);
  Future<void> save() async {}
  FutureOr<void> finish() {}
  int _internal() => state;
  static int utility() => 1;
  int get count => state;
  @override
  String toString() => 'counter';
  inferredVoid() => emit(1);
  inferredEmpty() {}
  outer() { int inner() => 1; inner(); }
  closureBoundary() { [1].map((value) { return value; }); }
  Never fail() => throw StateError('failure');
}
class EventBloc extends Bloc<int, int> {
  EventBloc() : super(0);
  int count() => state;
}
class Ordinary {
  int count() => 1;
}
abstract class ExternalApi {
  String describe();
}
class Implementation extends Cubit<int> implements ExternalApi {
  Implementation() : super(0);
  String describe() => 'implementation';
}
class FakeCubit implements Cubit<int> {
  int count() => 1;
}
class FakeCounter implements Counter {
  int extra() => 1;
}
