// @dart = 3.13
class PrimaryBase(final String issueType) {}
class PrimaryChild extends PrimaryBase {
  PrimaryChild() : super('promotion');
  @override
  String get issueType => 'hidden'; /* expect: avoid-suspicious-super-overrides */
}
