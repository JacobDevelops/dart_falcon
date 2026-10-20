import 'package:bloc/bloc.dart';

sealed class Event {}
final class Start extends Event {}
final class Stop extends Event {}

class CompleteBloc extends Bloc<Event, int> {
  CompleteBloc() : super(0) {
    on<Start>((event, emit) {});
    on<Stop>((event, emit) {});
  }
  CompleteBloc.parent() : super(0) {
    on<Event>((event, emit) {});
  }
  CompleteBloc.redirect() : this();
  CompleteBloc.inferred() : super(0) {
    on((Event event, Emitter<int> emit) {});
  }
  CompleteBloc.cascade() : super(0) {
    this
      ..on<Start>((event, emit) {})
      ..on<Stop>((event, emit) {});
  }
  CompleteBloc.inferredCascade() : super(0) {
    this..on((Event event, Emitter<int> emit) {});
  }
  CompleteBloc.cascadeWithLocalOn() : super(0) {
    void on<T>(Object handler) {}
    this
      ..on<Start>((event, emit) {})
      ..on<Stop>((event, emit) {});
  }
}
class NotABloc {
  NotABloc();
}

class GenericBloc<Event extends Object> extends Bloc<Event, int> {
  GenericBloc() : super(0) {
    on<Event>((event, emit) {});
  }
}

abstract class GenericEvent {}
abstract class Value<T> extends GenericEvent {}
class IntValue extends Value<int> {}

class MatchingValueBloc extends Bloc<GenericEvent, int> {
  MatchingValueBloc() : super(0) {
    on<Value<int>>((event, emit) {});
  }
  MatchingValueBloc.raw() : super(0) {
    on<Value>((event, emit) {});
  }
  MatchingValueBloc.dynamicArgument() : super(0) {
    on<Value<dynamic>>((event, emit) {});
  }
}

abstract class UnresolvedEvent {}
abstract class UnknownValue<T> extends UnresolvedEvent {}
class FooValue extends UnknownValue<Foo> {}

class UnresolvedValueBloc extends Bloc<UnresolvedEvent, int> {
  UnresolvedValueBloc() : super(0) {
    on<UnknownValue<Foo>>((event, emit) {});
  }
}

abstract class WrappedEvent {}
abstract class WrappedValue<T> extends WrappedEvent {}
class Wrapped<T> extends WrappedValue<T> {}

class WrappedValueBloc extends Bloc<WrappedEvent, int> {
  WrappedValueBloc() : super(0) {
    on<WrappedValue<int>>((event, emit) {});
  }
}

class HelperBloc extends Bloc<Event, int> {
  HelperBloc() : super(0) {
    _registerHandlers();
  }
  HelperBloc.explicitThis() : super(0) {
    this._registerHandlers();
  }
  void _registerHandlers() {
    on<Start>((event, emit) {});
    on<Stop>((event, emit) {});
  }
}

mixin HandlerRegistration on Bloc<Event, int> {
  void _registerHandlers() {
    on<Start>((event, emit) {});
    on<Stop>((event, emit) {});
  }
}

class MixinHelperBloc extends Bloc<Event, int> with HandlerRegistration {
  MixinHelperBloc() : super(0) {
    _registerHandlers();
  }
  MixinHelperBloc.explicitThis() : super(0) {
    this._registerHandlers();
  }
}
