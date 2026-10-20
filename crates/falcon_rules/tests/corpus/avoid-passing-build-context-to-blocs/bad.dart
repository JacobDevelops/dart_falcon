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
  Consumer(context); /* expect: avoid-passing-build-context-to-blocs */
  new Consumer(context); /* expect: avoid-passing-build-context-to-blocs */
  Consumer.named(dependency: context); /* expect: avoid-passing-build-context-to-blocs */
  ChildConsumer(context); /* expect: avoid-passing-build-context-to-blocs */
  ConsumerAlias(context); /* expect: avoid-passing-build-context-to-blocs */
  consumer.accept(context); /* expect: avoid-passing-build-context-to-blocs */
  ConsumerCubit(context); /* expect: avoid-passing-build-context-to-blocs */
  Consumer(custom); /* expect: avoid-passing-build-context-to-blocs */
  consumer..accept(context); /* expect: avoid-passing-build-context-to-blocs */
}

void eventPayload(f.BuildContext context, EventBloc bloc) {
  bloc.add(Event(context)); /* expect: avoid-passing-build-context-to-blocs */
  final event = ChildEvent(context); /* expect: avoid-passing-build-context-to-blocs */
  bloc.add(event);
}

void importedEvents(f.BuildContext context, CounterBloc bloc) {
  bloc.add(CounterEvent(context)); /* expect: avoid-passing-build-context-to-blocs */
}

class ImplicitConsumer extends b.Cubit<int> {
  ImplicitConsumer() : super(0);
  void accept(Object dependency) {}
  void dependencies(f.BuildContext context) {
    accept(context); /* expect: avoid-passing-build-context-to-blocs */
    this.accept(context); /* expect: avoid-passing-build-context-to-blocs */
  }
}

class InheritedConsumer extends ImplicitConsumer {
  void inherited(f.BuildContext context) {
    accept(context); /* expect: avoid-passing-build-context-to-blocs */
  }
}
