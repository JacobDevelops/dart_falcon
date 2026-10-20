import 'package:bloc/bloc.dart';
class Example extends Bloc<int, int> {
  Example() : super(0);
  void _command() {}
  static void shared() {}
  int get value => 1;
  @override Future<void> close() async {}
  @override void add(int event) {}
}
class Counter extends Cubit<int> { Counter() : super(0); void increment() {} }
class Ordinary { void command() {} }
