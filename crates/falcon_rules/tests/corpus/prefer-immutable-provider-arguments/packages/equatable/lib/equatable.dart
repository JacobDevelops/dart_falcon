abstract class Equatable {
  List<Object?> get props;
  bool operator ==(Object other) => identical(this, other);
  int get hashCode => 0;
}
mixin EquatableMixin {
  List<Object?> get props;
  bool operator ==(Object other) => identical(this, other);
  int get hashCode => 0;
}
