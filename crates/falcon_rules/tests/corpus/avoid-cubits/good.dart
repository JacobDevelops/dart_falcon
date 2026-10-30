import 'package:bloc/bloc.dart';
import 'package:bloc/bloc.dart' as b;

class Events extends Bloc<int, int> {
  Events() : super(0);
}
class InheritedEvents extends Events {}
class PrefixedEvents extends b.Bloc<String, int> {
  PrefixedEvents() : super(0);
}
class StateOnly extends BlocBase<int> {
  StateOnly() : super(0);
}
class Unrelated {}
class CubitFactory {
  Cubit<int> create() => Cubit<int>(0);
}
abstract class FakeCubit implements Cubit<int> {}
abstract class MockCounter implements b.Counter {}
abstract class IndirectMock extends MockCounter {}
