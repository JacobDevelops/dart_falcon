import 'package:bloc/bloc.dart';

void on<T>(Object handler) {}
class Event {}
class CounterBloc extends Bloc<Event, int> {
  CounterBloc() : super(0) {
    on<Event>((event, emit) {});
    on<Event>((event, emit) {});
  }
}
