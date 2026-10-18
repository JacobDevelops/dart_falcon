import 'package:bloc/bloc.dart';
import 'package:mixins/mixins.dart' show SafeHandlers;

class Event {}
class CounterBloc extends Bloc<Event, int> with SafeHandlers {
  CounterBloc() : super(0) {
    on<Event>((event, emit) {});
    on<Event>((event, emit) {});
  }
}
