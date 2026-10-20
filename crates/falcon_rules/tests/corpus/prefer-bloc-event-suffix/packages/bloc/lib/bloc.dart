class BlocBase<S> {
  BlocBase(this.state);
  final S state;
  void emit(S state) {}
  Future<void> close() async {}
}
class Bloc<E, S> extends BlocBase<S> {
  Bloc(S state) : super(state);
  void add(E event) {}
}
class Cubit<S> extends BlocBase<S> { Cubit(S state) : super(state); }
