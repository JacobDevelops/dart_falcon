import 'package:bloc/bloc.dart' as bloc;

class Event {}
void outside(bloc.Cubit<int> cubit, bloc.Bloc<Event, int> machine) {
  cubit.emit(1); /* expect: avoid-emit-outside-bloc */
  machine.emit(2); /* expect: avoid-emit-outside-bloc */
  final callback = cubit.emit; /* expect: avoid-emit-outside-bloc */
}
