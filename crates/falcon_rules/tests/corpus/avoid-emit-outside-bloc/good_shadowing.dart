import 'package:bloc/bloc.dart';

class Cubit<T> {
  void emit(T value) {}
}
class Bloc<E, S> {
  void emit(S value) {}
}
class LocalCubit extends Cubit<int> {}
class LocalBloc extends Bloc<int, int> {}
void local(LocalCubit cubit, LocalBloc bloc) {
  cubit.emit(1);
  bloc.emit(2);
}
