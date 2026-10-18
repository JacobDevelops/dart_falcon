class Cubit<T> {
  Cubit(this.state);
  T state;
  void emit(T value) {}
}
class Counter extends Cubit<int> {
  Counter() : super(0);
  void unchanged() { emit(state); }
}
