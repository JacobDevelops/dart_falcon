abstract class Cubit<T> {
  Cubit(this.state);
  T state;
  void emit(T value) { state = value; }
  Future<void> close() async {}
}
abstract class Bloc<E, S> {
  Bloc(this.state);
  S state;
}
