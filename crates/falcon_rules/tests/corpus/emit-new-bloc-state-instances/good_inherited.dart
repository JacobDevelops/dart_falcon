import 'package:bloc/bloc.dart';

class Event {}
class Emitter<T> { void call(T value) {} }
class CustomState extends Cubit<int> {
  CustomState() : super(0);
  int get state => 1;
}
class ChildState extends CustomState {
  void update() { emit(state); }
}
class CustomEmit extends Cubit<int> {
  CustomEmit() : super(0);
  void emit(int value) {}
}
class ChildEmit extends CustomEmit {
  void update() { emit(state); }
}
class CustomOn extends Bloc<Event, int> {
  CustomOn() : super(0);
  void on<T>(void Function(T, void Function(int)) handler) {}
}
class ChildOn extends CustomOn {
  ChildOn() { on<Event>((event, send) { send(state); }); }
}
class Counter extends Cubit<int> {
  Counter() : super(0);
  void on<T>(void Function(T, void Function(int)) handler) {}
  void setup() { on<Event>((event, send) { send(state); }); }
  void customEmitter(Event event, Emitter<int> emit) { emit(state); }
}
