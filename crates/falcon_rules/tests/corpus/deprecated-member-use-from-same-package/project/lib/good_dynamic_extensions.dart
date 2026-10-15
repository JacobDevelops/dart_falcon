extension DeprecatedObjectMembers on Object {
  @deprecated
  int get oldObjectProperty => 0;

  @deprecated
  void call() {}
}

extension DeprecatedNullableObjectMembers on Object? {
  @deprecated
  int get oldNullableObjectProperty => 0;
}

extension DeprecatedDynamicMembers on dynamic {
  @deprecated
  int get oldDynamicProperty => 0;
}

void dynamicDispatch(dynamic value) {
  value.oldObjectProperty;
  value.oldNullableObjectProperty;
  value.oldDynamicProperty;
  value();
}
