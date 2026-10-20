class Bloc<E, S> { Bloc(S state); }
class Cubit<S> { Cubit(S state); }
class Action {}
class Result {}
class Local extends Bloc<Action, Result> {
  Local() : super(Result());
  int visible = 0;
  void command() {}
}
class LocalCubit extends Cubit<Result> {
  LocalCubit() : super(Result());
  int visible = 0;
  void command() {}
}
