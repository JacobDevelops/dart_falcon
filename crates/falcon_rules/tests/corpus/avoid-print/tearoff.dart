void takesCallback(void Function(Object?) callback) {}
void takesNamed({required void Function(Object?) callback}) {}
void testPrintTearoff(List<Object?> values) {
  values.forEach(print); /* expect: avoid-print */
  takesCallback(print); /* expect: avoid-print */
  values..forEach(print); /* expect: avoid-print */
  values.forEach((print));
  takesNamed(callback: print);
  final callback = print;
  takesCallback(callback);
}
class CallbackHolder {
  CallbackHolder(void Function(Object?) callback);
  static void takes(void Function(Object?) callback) {}
  void Function(void Function(Object?)) field = takesCallback;
  void Function(void Function(Object?)) get getter => takesCallback;
  static void Function(void Function(Object?)) staticField = takesCallback;
  static void Function(void Function(Object?)) get staticGetter => takesCallback;
  void method(void Function(Object?) callback) {}
  void call(void Function(Object?) callback) {}
  void test() {
    field(print);
    getter(print);
    method(print); /* expect: avoid-print */
  }
}
typedef HolderAlias = CallbackHolder;
var topCallback = takesCallback;
void Function(void Function(Object?)) get topGetter => takesCallback;
void otherInvocations() {
  CallbackHolder(print);
  HolderAlias(print);
  (takesCallback)(print);
  CallbackHolder.takes(print); /* expect: avoid-print */
}
void callbackInvocations(
  void Function(void Function(Object?)) parameter,
  CallbackHolder holder,
) {
  var local = takesCallback;
  parameter(print);
  local(print);
  topCallback(print);
  topGetter(print);
  holder.field(print);
  holder.getter(print);
  CallbackHolder.staticField(print);
  CallbackHolder.staticGetter(print);
  holder.method(print); /* expect: avoid-print */
  holder.call(print); /* expect: avoid-print */
  holder(print);
  holder
    ..field(print)
    ..getter(print)
    ..method(print) /* expect: avoid-print */
    ..call(print); /* expect: avoid-print */
}
void lexicalConstructorShadow() {
  void CallbackHolder(void Function(Object?) callback) {}
  CallbackHolder(print); /* expect: avoid-print */
}
class MemberConstructorShadow {
  void CallbackHolder(void Function(Object?) callback) {}
  void test() {
    CallbackHolder(print); /* expect: avoid-print */
  }
}
class PrintBase {
  void print(Object? value) {}
}
class PrintChild extends PrintBase {
  void test(List<Object?> values) {
    values.forEach(print); /* expect: avoid-print */
    print('member'); /* expect: avoid-print */
  }
}
void shadowedPrint(void Function(Object?) print, List<Object?> values) {
  values.forEach(print);
}
