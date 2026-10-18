import 'package:bloc/bloc.dart';
import 'retry.dart';

class Event {}
class CounterBloc extends Bloc<Event, int> with RetryHandlers {
  CounterBloc() : super(0) {
    super.on<Event>((event, emit) {});
    super.on<Event>((event, emit) {});
  }
}
