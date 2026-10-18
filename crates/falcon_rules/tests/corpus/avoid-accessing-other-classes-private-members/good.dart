class Secret {
  int _value = 0;
  static int _shared = 0;
  int publicValue = 0;
  void _reset() {}
  void inspect(Secret other) {
    _value = other._value;
    this._reset();
    other.._value = 1;
    Secret._shared++;
    void nested() { other._reset(); }
    nested();
  }
}
class Other {
  int read(Secret secret) => secret.publicValue;
  void unknown(dynamic value) { value._unresolved(); }
}
class Child extends Secret {
  @override
  void _reset() { super._reset(); }
  int readOwn() => this._value;
  void resetOwn() { this.._value = 1; this.._reset(); }
}
extension NamedSecretAccess on Secret {
  int readOther(Secret other) => other._value;
}
extension on Secret {
  int readOtherUnnamed(Secret other) => other._value;
}
