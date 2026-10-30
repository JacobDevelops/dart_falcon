import 'package:bloc/bloc.dart';
import 'package:bloc/bloc.dart' as b;
import 'package:flutter_bloc/flutter_bloc.dart' as flutter;

class Counter extends Cubit<int> { /* expect: avoid-cubits */
  Counter() : super(0);
}
class Prefixed extends b.Cubit<int> { /* expect: avoid-cubits */
  Prefixed() : super(0);
}
abstract class Generic<T> extends Cubit<T> { /* expect: avoid-cubits */
  Generic(T state) : super(state);
}
class Indirect extends Counter {} /* expect: avoid-cubits */
class Deep extends Indirect {} /* expect: avoid-cubits */
class Reexported extends flutter.Cubit<int> { /* expect: avoid-cubits */
  Reexported() : super(0);
}
mixin Logging {}
class Named = Cubit<int> with Logging; /* expect: avoid-cubits */
class PrefixedNamed = b.Cubit<int> with Logging; /* expect: avoid-cubits */
class ExternalIndirect extends b.Counter {} /* expect: avoid-cubits */
