import 'package:bloc/bloc.dart';

sealed class Event {}
final class Start extends Event {}
final class Stop extends Event {}

class CompleteBloc extends Bloc<Event, int> {
  CompleteBloc() : super(0) {
    on<Start>((event, emit) {});
    on<Stop>((event, emit) {});
  }
  CompleteBloc.parent() : super(0) {
    on<Event>((event, emit) {});
  }
  CompleteBloc.redirect() : this();
  CompleteBloc.inferred() : super(0) {
    on((Event event, Emitter<int> emit) {});
  }
  CompleteBloc.cascade() : super(0) {
    this
      ..on<Start>((event, emit) {})
      ..on<Stop>((event, emit) {});
  }
  CompleteBloc.inferredCascade() : super(0) {
    this..on((Event event, Emitter<int> emit) {});
  }
  CompleteBloc.cascadeWithLocalOn() : super(0) {
    void on<T>(Object handler) {}
    this
      ..on<Start>((event, emit) {})
      ..on<Stop>((event, emit) {});
  }
}
class NotABloc {
  NotABloc();
}

class GenericBloc<Event extends Object> extends Bloc<Event, int> {
  GenericBloc() : super(0) {
    on<Event>((event, emit) {});
  }
}
