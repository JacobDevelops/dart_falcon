import 'dart:async';
import 'closed.dart';

base class Both with M implements Right {}
class NamedSymbol implements Symbol, Right {}
bool mixed(Closed value) => value is Right;
bool generic(Sealed value) => value is Box<int>;
bool symbolic(Symbol value) => value is Right;
bool number(FutureOr<int> value) => value is num;
bool doubleNumber(FutureOr<double> value) => value is num;
