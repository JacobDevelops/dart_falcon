import 'package:equatable/equatable.dart' as real;
class Equatable {
  List<Object?> get props => [];
}
mixin EquatableMixin {
  List<Object?> get props => [];
}
class LocalClass extends Equatable { int value = 1; }
class LocalMixin with EquatableMixin { int value = 1; }
class LocalDescendant extends LocalClass { int child = 2; }
class RealReference extends real.Equatable {
  final int value = 1;
  List<Object?> get props => [value];
}
