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

class DocumentationShadow {
  @deprecated
  void oldMethod() {}
  /// [oldMethod] is the parameter, not the deprecated member.
  void accept(void Function() oldMethod) {}
}
/// [oldFunction] is the parameter, not the deprecated top-level declaration.
void documentedParameter(void Function() oldFunction) {}
