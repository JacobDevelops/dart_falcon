import 'package:bloc/bloc.dart';

class Counter extends Cubit<int> {
  Counter() : super(0);
}
void listen(void Function(int) callback) {}
void outside(Counter counter) {
  listen(counter.emit); /* expect: avoid-emit-outside-bloc */
  final callback = counter.emit; /* expect: avoid-emit-outside-bloc */
}
