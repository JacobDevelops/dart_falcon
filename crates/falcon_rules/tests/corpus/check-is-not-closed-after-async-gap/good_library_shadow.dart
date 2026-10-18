import 'package:bloc/bloc.dart';

void add(Object event) {}
class CounterBloc extends Bloc<int, int> {
  CounterBloc() : super(0);
  Future<void> load() async {
    await Future<void>.value();
    add(1);
  }
}
