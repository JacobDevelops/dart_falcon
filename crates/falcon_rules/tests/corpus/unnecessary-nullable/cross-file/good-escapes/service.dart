part 'caller.dart';

void accept(void Function(int?) callback) { callback(null); }

class Service {
  void _qualified(int? value) {}
  void _bare(int? value) {}
  void _cascade(int? value) {}
  void _cascadeField(int? value) {}

  void exercise() {
    _qualified(1);
    _bare(1);
    _cascade(1);
    _cascadeField(1);
    accept(this._qualified);
    accept(_bare);
    this.._cascade.call(null);
    this.._cascadeField;
  }
}

int? maybeNull() => null;

void _writeCall(int? value) { value = maybeNull(); }
void _writeVariable(int? value) { int? other = maybeNull(); value = other; }
void _writeConditional(int? value) { value = value == 1 ? 1 : null; }
void _nullableArgument(int? value) {}

void _writePattern(int? value) { [value] = <int?>[null]; }
void _writeLoop(int? value) { for (value in <int?>[null]) {} }
void _writeCollectionLoop(int? value) {
  final values = [for (value in <int?>[null]) value];
}
const int? none = null;
void _optionalDefault([int? value = none]) {}
void _namedDefault({int? value = none}) {}
void _orderedArguments(int first, int? second) {}

class Factory {
  static Factory _make(int? value) => Factory();
  static void exercise() {
    Factory instance = ._make(null);
    _make(1);
  }
}

void _writeExpression(int? value) {
  final result = ([value] = <int?>[null]);
}
void _writeMapLoop(int? value) {
  final values = {for (value in <int?>[null]) 1: 1};
}

void _genericArgument(Object? value) {}
void _genericWrite<T>(Object? value, T other) { value = other; }
void _genericLoop<T>(Object? value, Iterable<T> values) {
  for (value in values) {}
}
void _unknownLoop(int? value, dynamic values) {
  for (value in values) {}
}
