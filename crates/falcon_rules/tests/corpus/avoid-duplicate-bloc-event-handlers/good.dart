import 'package:bloc/bloc.dart';

sealed class Event {}
final class Start extends Event {}
final class Stop extends Event {}

class CounterBloc extends Bloc<Event, int> {
  CounterBloc() : super(0) {
    on<Start>((event, emit) {});
    on<Stop>((event, emit) {});
  }
  CounterBloc.named() : super(0) {
    on<Start>((event, emit) {});
  }
  CounterBloc.other(CounterBloc other) : super(0) {
    on<Start>((event, emit) {});
    other.on<Start>((event, emit) {});
  }
  CounterBloc.local(void Function<T>(Object callback) on) : super(0) {
    on<Start>((event, emit) {});
    on<Start>((event, emit) {});
  }
  CounterBloc.branch(bool flag) : super(0) {
    if (flag) {
      on<Start>((event, emit) {});
    } else {
      on<Start>((event, emit) {});
    }
  }
}

class Registry {
  Registry() {
    on<Start>(() {});
    on<Start>(() {});
  }
  void on<T>(Object callback) {}
}

class CustomBloc extends Bloc<Event, int> {
  CustomBloc() : super(0) {
    on<Start>(() {});
    on<Start>(() {});
  }
  void on<T>(Object callback) {}
}

class InheritedCustomBloc extends CustomBloc {
  InheritedCustomBloc() : super() {
    on<Start>(() {});
    on<Start>(() {});
  }
}
