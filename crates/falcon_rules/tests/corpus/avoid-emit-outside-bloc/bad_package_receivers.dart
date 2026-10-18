import 'package:bloc/bloc.dart';

void outside(Cubit<int> source, Cubit<int>? nullable) {
  source..emit(1); /* expect: avoid-emit-outside-bloc */
  source..emit; /* expect: avoid-emit-outside-bloc */
  nullable!.emit(2); /* expect: avoid-emit-outside-bloc */
  (nullable!)..emit(3); /* expect: avoid-emit-outside-bloc */
  Cubit<int> local = source;
  local.emit(4); /* expect: avoid-emit-outside-bloc */
  local..emit(5); /* expect: avoid-emit-outside-bloc */
}
class Presenter {
  final Cubit<int> counter;
  Presenter(this.counter);
  void update() {
    counter.emit(6); /* expect: avoid-emit-outside-bloc */
    this.counter.emit(7); /* expect: avoid-emit-outside-bloc */
    counter..emit(8); /* expect: avoid-emit-outside-bloc */
  }
}
extension OutsideCubit on Cubit<int> {
  void updateOutside() {
    emit(9); /* expect: avoid-emit-outside-bloc */
    this.emit(10); /* expect: avoid-emit-outside-bloc */
    this..emit(11); /* expect: avoid-emit-outside-bloc */
    final callback = this.emit; /* expect: avoid-emit-outside-bloc */
  }
}
extension on Cubit<int> {
  void anonymousOutside() {
    emit(12); /* expect: avoid-emit-outside-bloc */
    this.emit(13); /* expect: avoid-emit-outside-bloc */
  }
}
