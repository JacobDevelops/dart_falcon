import 'package:bloc/bloc.dart';
import 'retry.dart';

class CounterCubit extends Cubit<int> with RetryEmit {
  CounterCubit() : super(0);
  void send() {
    super.emit(state);
    emit(super.state);
  }
}
