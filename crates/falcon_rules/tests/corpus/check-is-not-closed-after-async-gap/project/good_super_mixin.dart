import 'package:bloc/bloc.dart';
import 'retry.dart';

class CounterCubit extends Cubit<int> with RetryEmit {
  CounterCubit() : super(0);
  Future<void> send() async {
    await Future<void>.value();
    super.emit(1);
  }
}
