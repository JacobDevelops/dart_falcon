import 'package:bloc/bloc.dart';
class Example extends Bloc<int, int> {
  Example() : super(0);
  final int _value = 0;
  static final int shared = 1;
  int get value => _value;
  @override final int state = 0;
  void _command() {}
}
class Ordinary { int value = 0; }
