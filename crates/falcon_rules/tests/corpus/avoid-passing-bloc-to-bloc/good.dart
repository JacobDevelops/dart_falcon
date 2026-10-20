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

void accept(Object value) {}

class ShadowedConsumer extends real.Consumer {
  ShadowedConsumer() : super(0);
  void localVariable(real.Counter counter) {
    final accept = (Object value) {};
    accept(counter);
  }
  void localFunction(real.Counter counter) {
    void accept(Object value) {}
    accept(counter);
  }
  void parameter(real.Counter counter, void Function(Object) accept) {
    accept(counter);
  }
  void libraryFunction(real.Counter counter) {
    accept(counter);
  }
  static void helper(Object value) {}
  void staticFunction(real.Counter counter) {
    helper(counter);
  }
}
