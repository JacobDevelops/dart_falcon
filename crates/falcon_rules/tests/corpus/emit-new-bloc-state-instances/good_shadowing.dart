import 'package:bloc/bloc.dart';

class Cubit<T> {
  Cubit(this.state);
  T state;
  void emit(T value) {}
}
class LocalCubit extends Cubit<int> {
  LocalCubit() : super(0);
  void unchanged() { emit(state); this.emit(this.state); }
}
