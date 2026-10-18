class Base {
  final String issueType;
  const Base({required this.issueType});
  const Base.named(this.issueType);
}
class Child extends Base {
  Child() : super(issueType: 'promotion');
  @override
  String get issueType => 'hidden'; /* expect: avoid-suspicious-super-overrides */
}
class Positional extends Base {
  Positional() : super.named('promotion');
  @override
  String get issueType => 'hidden'; /* expect: avoid-suspicious-super-overrides */
}
class Forwarded extends Base {
  Forwarded({required super.issueType});
  @override
  String get issueType => 'hidden'; /* expect: avoid-suspicious-super-overrides */
}
class PositionalForwarded extends Base {
  PositionalForwarded(super.issueType) : super.named();
  @override
  String get issueType => 'hidden'; /* expect: avoid-suspicious-super-overrides */
}
