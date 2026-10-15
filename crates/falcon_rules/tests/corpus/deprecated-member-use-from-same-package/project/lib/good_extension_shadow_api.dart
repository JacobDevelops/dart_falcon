extension ShadowedExtension on String {
  @deprecated
  void old() {}
  @deprecated
  static void oldStatic() {}
  static void oldPositional([@deprecated int? value]) {}
  static void oldNamed({@deprecated int? value}) {}
}
