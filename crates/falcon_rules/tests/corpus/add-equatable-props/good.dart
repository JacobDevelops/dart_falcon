import 'package:equatable/equatable.dart';
import 'package:equatable/equatable.dart' as eq;
import 'support.dart';
class Direct extends Equatable { List<Object?> get props => []; }
class MixedValue with EquatableMixin { List<Object?> get props => []; }
class Indirect extends Parent { List<Object?> get props => super.props; }
class Transitive extends Middle { List<Object?> get props => []; }
class Prefixed extends eq.Equatable { List<Object?> get props => []; }
class MixedChild extends Mixed { List<Object?> get props { return []; } }
class Unrelated { final int value = 1; }
