// @dart = 3.13
import 'package:riverpod/riverpod.dart' as rp;

void initializeInterface() {}
class Fake {}
abstract class CounterContract extends rp.Notifier<int> {}
class ImplementsOnly extends Fake implements CounterContract {
  late int state;
  ImplementsOnly() { initializeInterface(); }
}
abstract class InterfaceContract implements rp.Notifier<int> {}
class IndirectImplementsOnly extends Fake implements InterfaceContract {
  late int state;
  IndirectImplementsOnly() { initializeInterface(); }
}
class PrimaryNoFields(int seed) extends rp.Notifier<int> {}
