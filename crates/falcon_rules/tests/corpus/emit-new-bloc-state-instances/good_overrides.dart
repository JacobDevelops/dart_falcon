import 'package:bloc/bloc.dart';

class CustomEmission extends Cubit<int> {
  CustomEmission() : super(0);
  @override
  void emit(int value) {}
  void process() { emit(state); }
}
class CustomState extends Cubit<int> {
  CustomState() : super(0);
  @override
  int get state => 42;
  void process() { emit(state); }
}
