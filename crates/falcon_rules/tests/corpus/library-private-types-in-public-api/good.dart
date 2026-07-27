class _Private {
  _Private field;
  _Private(this.field);
  _Private method(_Private value) => value;
}

_Private _privateFunction(_Private value) => value;
_Private _privateVariable = _Private(_Private(_Private(null as dynamic)));
typedef _PrivateAlias = _Private Function(_Private);

class PublicClass {
  String field;
  PublicClass(this.field);
  String method(String value) => value;
  _Private _privateMethod(_Private value) => value;
}

typedef PublicAlias = ({String value, void Function(String) callback});
extension PublicExtension on String {}
extension on _Private {}
extension type PublicExtensionType._(_Private _value) {}

enum PublicEnum {
  value._(null);
  const PublicEnum._(_Private? value);
}

void publicNamedParameter({String? value}) {}
void _privateNamedParameter({_Private? value}) {}
