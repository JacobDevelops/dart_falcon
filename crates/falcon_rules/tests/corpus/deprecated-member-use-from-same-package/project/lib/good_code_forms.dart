class ScopeForms {
  @deprecated
  int field = 0;
  @deprecated
  int get oldValue => 0;
  set oldValue(int value) {}
  void shadow(int field) {
    field;
    field = 1;
  }
  void setterOnly() {
    oldValue = 1;
  }
  @deprecated
  void exempt() {
    field;
    field += 1;
  }
}
void requiredPositional(@deprecated int old) {}
void requiredNamed({@deprecated required int old}) {}
void optionalBody([@deprecated int old = 0]) { old; }
void silentForms(dynamic unknown) {
  unknown.field;
  unknown.field = 1;
  unknown();
  requiredPositional(1);
  requiredNamed(old: 1);
  optionalBody();
  @deprecated
  void local() {}
  {
    final local = () {};
    local();
  }
}
extension UndeprecatedForms on String {
  void current() {}
}
void currentExtension() {
  ''.current();
  UndeprecatedForms('').current();
}
