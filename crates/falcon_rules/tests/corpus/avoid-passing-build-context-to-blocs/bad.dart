import 'package:flutter/widgets.dart' as f;
import 'support.dart';
import 'package:bloc/bloc.dart' as b;

class ConsumerCubit extends b.Cubit<int> {
  ConsumerCubit(Object dependency) : super(0);
}

class CustomContext implements f.BuildContext {
  bool get mounted => true;
}

typedef ConsumerAlias = Consumer;

class Event {
  Event(Object payload);
}
class ChildEvent extends Event {
  ChildEvent(Object payload) : super(payload);
}
class EventBloc extends b.Bloc<Event, int> {
  EventBloc() : super(0);
  void add(Event event) {}
}

void dependencies(f.BuildContext context, Consumer consumer, CustomContext custom) {
  Consumer(/* expect: avoid-passing-build-context-to-blocs */ context);
  new Consumer(/* expect: avoid-passing-build-context-to-blocs */ context);
  Consumer.named(dependency: /* expect: avoid-passing-build-context-to-blocs */ context);
  ChildConsumer(/* expect: avoid-passing-build-context-to-blocs */ context);
  ConsumerAlias(/* expect: avoid-passing-build-context-to-blocs */ context);
  consumer.accept(/* expect: avoid-passing-build-context-to-blocs */ context);
  ConsumerCubit(/* expect: avoid-passing-build-context-to-blocs */ context);
  Consumer(/* expect: avoid-passing-build-context-to-blocs */ custom);
  consumer..accept(/* expect: avoid-passing-build-context-to-blocs */ context);
}

void eventPayload(f.BuildContext context, EventBloc bloc) {
  bloc.add(Event(/* expect: avoid-passing-build-context-to-blocs */ context));
  final event = ChildEvent(/* expect: avoid-passing-build-context-to-blocs */ context);
  bloc.add(event);
}

void importedEvents(f.BuildContext context, CounterBloc bloc) {
  bloc.add(CounterEvent(/* expect: avoid-passing-build-context-to-blocs */ context));
}

class ImplicitConsumer extends b.Cubit<int> {
  ImplicitConsumer() : super(0);
  void accept(Object dependency) {}
  void dependencies(f.BuildContext context) {
    accept(/* expect: avoid-passing-build-context-to-blocs */ context);
    this.accept(/* expect: avoid-passing-build-context-to-blocs */ context);
  }
}

class InheritedConsumer extends ImplicitConsumer {
  void inherited(f.BuildContext context) {
    accept(/* expect: avoid-passing-build-context-to-blocs */ context);
  }
}
