// @dart = 3.6
// Official wildcard_featureDisabledFails: underscore is a real parameter.
class LegacyBase {
  void method(int value) {}
}
class LegacyChild extends LegacyBase {
  void method(int _) {} /* expect: avoid-renaming-method-parameters */
}
