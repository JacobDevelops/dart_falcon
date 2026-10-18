import 'package:bloc/bloc.dart';

class Counter extends Cubit<int> { Counter() : super(0); }
void outside(Counter counter) {
  counter..emit; /* expect: avoid-emit-outside-bloc */
}
