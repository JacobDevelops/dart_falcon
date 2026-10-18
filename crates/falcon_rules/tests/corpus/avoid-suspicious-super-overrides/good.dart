class Base {
  final String issueType;
  const Base({required this.issueType});
}
class NoOverride extends Base {
  NoOverride() : super(issueType: 'promotion');
}
class UsesSuper extends Base {
  UsesSuper() : super(issueType: 'promotion');
  @override
  String get issueType => '${super.issueType}: detail';
}
class DefaultBase {
  final String issueType;
  DefaultBase({this.issueType = 'default'});
}
class NoPassedValue extends DefaultBase {
  @override
  String get issueType => 'different';
}
class GetterBase {
  GetterBase({required String issueType});
  String get issueType => 'base';
}
class OverridesGetter extends GetterBase {
  OverridesGetter() : super(issueType: 'promotion');
  @override
  String get issueType => 'child';
}
class UnrelatedParameterBase {
  final String issueType = 'stored';
  UnrelatedParameterBase({required String issueType});
}
class UnrelatedParameter extends UnrelatedParameterBase {
  UnrelatedParameter() : super(issueType: 'not stored');
  @override
  String get issueType => 'child';
}
