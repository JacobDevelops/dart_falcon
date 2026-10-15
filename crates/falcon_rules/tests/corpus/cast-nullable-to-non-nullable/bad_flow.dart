void branchAndAssignment(int? value, int? replacement) {
  value as int; /* expect: cast-nullable-to-non-nullable */
  if (value != null) {
    value as int;
    value = replacement;
    value as int; /* expect: cast-nullable-to-non-nullable */
  }
}
void earlyReturnAndShadow(int? value) {
  if (value == null) return;
  value as int;
  {
    int? value;
    value as int; /* expect: cast-nullable-to-non-nullable */
  }
  value as int;
}
void castPromotes(int? value) {
  value as int; /* expect: cast-nullable-to-non-nullable */
  value as int;
}
class StableField {
  final int? _stable;
  StableField(this._stable);
  void checked(StableField other, StableField replacement) {
    if (other._stable != null) {
      other._stable as int;
      other = replacement;
      other._stable as int; /* expect: cast-nullable-to-non-nullable */
    }
    if (this._stable != null) _stable as int;
  }
}
void closureWrite(int? value, int? replacement) {
  void change() { value = replacement; }
  if (value != null) {
    value as int; /* expect: cast-nullable-to-non-nullable */
    change();
    value as int; /* expect: cast-nullable-to-non-nullable */
  }
}

class MutableField {
  int? _mutable;
  MutableField(this._mutable);
  void checked() {
    if (_mutable != null) {
      _mutable as int; /* expect: cast-nullable-to-non-nullable */
    }
  }
}
class ConflictingField {
  final int? _conflicting;
  ConflictingField(this._conflicting);
  void checked(ConflictingField other) {
    if (other._conflicting != null) {
      other._conflicting as int; /* expect: cast-nullable-to-non-nullable */
    }
  }
}
class ConflictingGetter {
  int? get _conflicting => null;
}

void nullableLoop(int? value, int? replacement) {
  while (value != null) {
    value as int;
    value = replacement;
    value as int; /* expect: cast-nullable-to-non-nullable */
  }
  value as int; /* expect: cast-nullable-to-non-nullable */
}
