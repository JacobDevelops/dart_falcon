class Cubit<T> { void emit(T value) {} }
class Counter extends Cubit<int> {}
void unchanged(Counter counter) { counter.emit(0); }
