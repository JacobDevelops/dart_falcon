import 'package:flutter/widgets.dart' as flutter;
import 'support.dart' as real;

class BuildContext {}
class Bloc<E, S> {}
class LocalBloc extends Bloc<Object, int> {
  LocalBloc(Object dependency);
}

class LookupConsumer extends real.Consumer {
  LookupConsumer() : super(0);
  static LookupConsumer of(flutter.BuildContext context) => LookupConsumer();
}
class UnrelatedEvent {
  UnrelatedEvent(Object payload);
}

void boundaries(flutter.BuildContext context, real.Consumer consumer, dynamic unknown) {
  real.Consumer(BuildContext());
  real.Consumer(real.Repository(0));
  real.Consumer(unknown);
  real.Repository(context);
  real.View(context);
  LocalBloc(context);
  consumer.accept(context.mounted);
  consumer..accept(context.mounted);
  LookupConsumer.of(context);
  UnrelatedEvent(context);
  final Consumer = (Object dependency) => real.Repository(dependency);
  Consumer(context);
}
