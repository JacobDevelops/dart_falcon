part of 'service.dart';

void use(Service service) {
  service.exercise();
  _writeCall(1);
  _writeVariable(1);
  _writeConditional(1);
  _nullableArgument(maybeNull());
}

void additionalCalls() {
  _writePattern(1);
  _writeLoop(1);
  _writeCollectionLoop(1);
  _optionalDefault();
  _namedDefault();
  int? value = maybeNull();
  if (value == null) return;
  _orderedArguments((value = null) == null ? 1 : 2, value);
  _writeExpression(1);
  _writeMapLoop(1);
}

void genericCalls<T>(T value, Iterable<T> values) {
  _genericArgument(value);
  _genericWrite(1, value);
  _genericLoop(1, values);
  _unknownLoop(1, values);
}
