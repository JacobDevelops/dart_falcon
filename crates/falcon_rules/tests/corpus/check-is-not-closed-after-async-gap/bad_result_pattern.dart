import 'package:bloc/bloc.dart';

class Ok {
  final int value;
  Ok(this.value);
}
class CounterCubit extends Cubit<int> {
  CounterCubit() : super(0);
  Future<void> run() async {
    final result = await load();
    if (result case Ok(:final value)) {
      emit(value); /* expect: check-is-not-closed-after-async-gap */
    }
  }
  Future<Object> load() async => Ok(1);
}
