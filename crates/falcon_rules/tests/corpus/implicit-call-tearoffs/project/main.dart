import 'api.dart';

void check(Callable callable, StaticCallable staticCallable, Provider provider) {
  accept(callable); /* expect: implicit-call-tearoffs */
  void Function() returned = make(); /* expect: implicit-call-tearoffs */
  void Function() member = provider.create(); /* expect: implicit-call-tearoffs */
  Sink(callable); /* expect: implicit-call-tearoffs */
  accept(callable.call);
  accept(staticCallable);
}

void shadow(Callable callable) {
  void accept(Object value) {}
  accept(callable);
}
