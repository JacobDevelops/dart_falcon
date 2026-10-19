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

class MixedBase {
  final int count;
  final String issueType;
  MixedBase(this.count, {this.issueType = 'default'});
}
class PositionalNameOnly extends MixedBase {
  PositionalNameOnly(super.issueType);
  @override
  String get issueType => 'hidden';
}

class InitializedBase {
  final String issueType;
  InitializedBase(String issueType) : issueType = issueType;
}
class InitializedChild extends InitializedBase {
  InitializedChild() : super('promotion');
  @override
  String get issueType => 'hidden';
}
class Intermediate extends Base {
  Intermediate({required super.issueType});
}
class IndirectChild extends Intermediate {
  IndirectChild() : super(issueType: 'promotion');
  @override
  String get issueType => 'hidden';
}
