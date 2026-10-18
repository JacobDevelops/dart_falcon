import 'package:bloc/bloc.dart';
import 'package:mixins/mixins.dart' show SafeEmitMixin;

class CounterCubit extends Cubit<int> with SafeEmitMixin {
  CounterCubit() : super(0);
  Future<void> load() async {
    await Future<void>.value();
    emit(1);
  }
}
