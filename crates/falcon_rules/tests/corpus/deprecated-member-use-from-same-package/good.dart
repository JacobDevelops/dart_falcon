@deprecated
void oldFunction() { oldFunction(); }
void shadow(void Function() oldFunction) { oldFunction(); }
void localShadow() { void oldFunction() {} oldFunction(); }
void unknown(dynamic value) { value.oldMethod(); }
void freshFunction() {}
void fresh() { freshFunction(); }
@Deprecated('Use freshFunction')
void exempt() { oldFunction(); }

void requiredNamed({@deprecated required int value}) {}

class RequiredNamedConstructor {
  RequiredNamedConstructor({@Deprecated('Still required') required int value});
}

void requiredNamedUses() {
  requiredNamed(value: 1);
  RequiredNamedConstructor(value: 1);
}
