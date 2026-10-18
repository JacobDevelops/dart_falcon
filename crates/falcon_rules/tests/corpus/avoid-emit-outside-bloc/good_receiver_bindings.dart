import 'package:bloc/bloc.dart';

class Sink { void emit(int value) {} }
class Presenter {
  final Cubit<int> counter;
  Presenter(this.counter);
  void parameter(Sink counter) { counter.emit(1); }
  void local() {
    final counter = Sink();
    counter.emit(2);
    counter..emit(3);
  }
}
void nested(Cubit<int> counter) {
  void unrelated(Sink counter) { counter.emit(1); }
  unrelated(Sink());
}
void cascadeMember(Cubit<Sink> counter) {
  counter..state.emit(1);
}
