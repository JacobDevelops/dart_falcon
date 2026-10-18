import 'package:bloc/bloc.dart';

sealed class Event {}
final class Start extends Event {}
final class Stop extends Event {}
final class Reset extends Event {}

class CounterBloc extends Bloc<Event, int> {
  CounterBloc() : super(0) {
    on<Start>((event, emit) {});
    on<Start>((event, emit) {}); /* expect: avoid-duplicate-bloc-event-handlers */
    this.on<Start>((event, emit) {}); /* expect: avoid-duplicate-bloc-event-handlers */
    on<Stop>((event, emit) {});
    this.on<Stop>((event, emit) {}); /* expect: avoid-duplicate-bloc-event-handlers */
    on<Reset>((event, emit) {});
    on<Reset>((event, emit) {}); /* expect: avoid-duplicate-bloc-event-handlers */
  }
  CounterBloc.named() : super(0) {
    on<Start>((event, emit) {});
    on<Start>((event, emit) {}); /* expect: avoid-duplicate-bloc-event-handlers */
  }
}
