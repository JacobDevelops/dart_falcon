import 'support.dart' as real;

class Bloc<E, S> {}
class Cubit<S> {}
class LocalBloc extends Bloc<Object, int> {
  LocalBloc(Object dependency);
}

class LookupConsumer extends real.Consumer {
  LookupConsumer() : super(0);
  static LookupConsumer of(real.Counter counter) => LookupConsumer();
}

void boundaries(real.Counter counter, real.Consumer consumer, dynamic unknown) {
  real.Consumer(real.Repository(0));
  real.Repository(counter);
  real.View(counter);
  real.Consumer(Cubit<int>());
  real.Consumer(Bloc<Object, int>());
  real.Consumer(unknown);
  LocalBloc(counter);
  consumer.accept(1);
  consumer..accept(1);
  LookupConsumer.of(counter);
  final Consumer = (Object dependency) => real.Repository(dependency);
  Consumer(counter);
}
