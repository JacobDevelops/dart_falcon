import 'support.dart';
import 'package:bloc/bloc.dart' as b;

typedef ConsumerAlias = Consumer;

class ConsumerCubit extends b.Cubit<int> {
  ConsumerCubit(Object dependency) : super(0);
}

void dependencies(Counter counter, Consumer consumer, b.Cubit<int> cubit) {
  Consumer(counter); /* expect: avoid-passing-bloc-to-bloc */
  new Consumer(counter); /* expect: avoid-passing-bloc-to-bloc */
  Consumer.named(dependency: cubit); /* expect: avoid-passing-bloc-to-bloc */
  ChildConsumer(counter); /* expect: avoid-passing-bloc-to-bloc */
  ConsumerAlias(counter); /* expect: avoid-passing-bloc-to-bloc */
  consumer.accept(counter); /* expect: avoid-passing-bloc-to-bloc */
  Consumer(Counter()); /* expect: avoid-passing-bloc-to-bloc */
  ConsumerCubit(counter); /* expect: avoid-passing-bloc-to-bloc */
  consumer..accept(counter); /* expect: avoid-passing-bloc-to-bloc */
}

class ImplicitConsumer extends b.Cubit<int> {
  ImplicitConsumer() : super(0);
  void accept(Object dependency) {}
  void dependencies(Counter counter) {
    accept(counter); /* expect: avoid-passing-bloc-to-bloc */
    this.accept(counter); /* expect: avoid-passing-bloc-to-bloc */
  }
}

class InheritedConsumer extends ImplicitConsumer {
  void inherited(Counter counter) {
    accept(counter); /* expect: avoid-passing-bloc-to-bloc */
  }
}
