dynamic globalValue;
Function globalCallback = () => null;

abstract class DynamicCallback {
  void accept(dynamic value);
}

class UntypedDynamicCallback implements DynamicCallback {
  @override
  void accept(value) {
    value.handle(); /* expect: avoid-dynamic-calls */
  }
}

class CallableHolder {
  Function callback;
  CallableHolder(this.callback);
  Function get getterCallback => callback;
}

void indexedValues(List<dynamic> list, Map<String, dynamic> map) {
  list[0].handle(); /* expect: avoid-dynamic-calls */
  map['key'].handle(); /* expect: avoid-dynamic-calls */
}

void globals() {
  globalValue.handle(); /* expect: avoid-dynamic-calls */
  final result = globalCallback(); /* expect: avoid-dynamic-calls */
  result.handle(); /* expect: avoid-dynamic-calls */
}

void callableResults(CallableHolder holder, dynamic receiver) {
  final fieldResult = holder.callback(); /* expect: avoid-dynamic-calls */
  fieldResult.handle(); /* expect: avoid-dynamic-calls */
  final getterResult = holder.getterCallback(); /* expect: avoid-dynamic-calls */
  getterResult.handle(); /* expect: avoid-dynamic-calls */
  final memberResult = receiver.make(); /* expect: avoid-dynamic-calls */
  memberResult.handle(); /* expect: avoid-dynamic-calls */
}

void inferredDynamicLocalReturn(dynamic value) {
  local() => value;
  local().handle(); /* expect: avoid-dynamic-calls */
}

class Replacements {
  Map<String, Function>? _replacements;
  Map<String, Function> get replacements => _replacements ??= {};

  void apply(String key, Map<String, Function> local) {
    local['x']!(); /* expect: avoid-dynamic-calls */
    replacements[key]!(); /* expect: avoid-dynamic-calls */
  }
}

void conditionalCallbacks(bool flag, Function first, Function second) {
  final response = flag ? first() : second(); /* expect: avoid-dynamic-calls */ /* expect: avoid-dynamic-calls */
  response.handle(); /* expect: avoid-dynamic-calls */
}
