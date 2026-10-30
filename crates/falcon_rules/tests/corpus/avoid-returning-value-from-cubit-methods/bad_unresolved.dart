import 'package:bloc/unavailable.dart';

class Counter extends Cubit<int> {}
class Indirect extends Counter {
  int value() => 1; /* expect: avoid-returning-value-from-cubit-methods */
  void command() {}
}
class Deep extends Indirect {
  String label() => 'counter'; /* expect: avoid-returning-value-from-cubit-methods */
}
mixin Logging {}
class Named = Counter with Logging;
class NamedChild extends Named {
  int count() => 1; /* expect: avoid-returning-value-from-cubit-methods */
}
class EventBloc extends Bloc<int, int> {
  int value() => 1;
}
class Contract implements Counter {
  int value() => 1;
}
class ContractChild extends Contract {
  int count() => 1;
}
