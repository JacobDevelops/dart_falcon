import 'package:bloc/unavailable.dart';

class Counter extends Cubit<int> {} /* expect: avoid-cubits */
class Indirect extends Counter {} /* expect: avoid-cubits */
class Deep extends Indirect {} /* expect: avoid-cubits */
mixin Logging {}
class Named = Counter with Logging; /* expect: avoid-cubits */
class NamedChild extends Named {} /* expect: avoid-cubits */
class EventBloc extends Bloc<int, int> {}
class Contract implements Counter {}
class ContractChild extends Contract {}
