class Bloc<E, S> {
  Bloc(S state);
  void on<T>(void Function(T, Object) handler) {}
}
sealed class Event {}
final class Start extends Event {}
final class Stop extends Event {}
class LocalBloc extends Bloc<Event, int> {
  LocalBloc() : super(0) {
    on<Start>((event, emit) {});
  }
}
