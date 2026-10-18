import 'package:equatable/equatable.dart' as eq;
class First with eq.EquatableMixin {}
class Second extends First {}
class Third implements eq.Equatable {}
class Fourth {}
class Equatable {}
class Local extends Equatable {}
class Unknown extends Missing {}
mixin NeedsEquatable on eq.Equatable {}
class Constrained extends eq.Equatable with NeedsEquatable {}
class ConstrainedAlias = eq.Equatable with NeedsEquatable;
