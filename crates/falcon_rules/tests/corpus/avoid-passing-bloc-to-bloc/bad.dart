import 'support.dart';
import 'package:bloc/bloc.dart' as b;

typedef ConsumerAlias = Consumer;

class ConsumerCubit extends b.Cubit<int> {
  ConsumerCubit(Object dependency) : super(0);
}

void dependencies(Counter counter, Consumer consumer, b.Cubit<int> cubit) {
  Consumer(/* expect: avoid-passing-bloc-to-bloc */ counter);
  new Consumer(/* expect: avoid-passing-bloc-to-bloc */ counter);
  Consumer.named(dependency: /* expect: avoid-passing-bloc-to-bloc */ cubit);
  ChildConsumer(/* expect: avoid-passing-bloc-to-bloc */ counter);
  ConsumerAlias(/* expect: avoid-passing-bloc-to-bloc */ counter);
  consumer.accept(/* expect: avoid-passing-bloc-to-bloc */ counter);
  Consumer(/* expect: avoid-passing-bloc-to-bloc */ Counter());
  ConsumerCubit(/* expect: avoid-passing-bloc-to-bloc */ counter);
  consumer..accept(/* expect: avoid-passing-bloc-to-bloc */ counter);
}
