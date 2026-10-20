abstract class BlocBase<S> {
  BlocBase(S state);
}
class Cubit<S> extends BlocBase<S> {
  Cubit(S state) : super(state);
}
class Bloc<E, S> extends BlocBase<S> {
  Bloc(S state) : super(state);
}
