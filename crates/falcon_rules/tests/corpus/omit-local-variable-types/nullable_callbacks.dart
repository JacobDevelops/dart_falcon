typedef NullableIndex = int? Function(String key);
class CallbackWidget {
  final NullableIndex? findChildIndexCallback;
  final int Function(int value)? callback;
  const CallbackWidget(this.findChildIndexCallback, this.callback);
}
abstract class CallbackState<W> {
  W get widget => throw 0;
}
abstract class CallbackConsumer extends CallbackState<CallbackWidget> {
  void use(String key, int value) {
    final int? index = widget.findChildIndexCallback!(key); /* expect: omit-local-variable-types */
    final int? explicit = widget.findChildIndexCallback!.call(key); /* expect: omit-local-variable-types */
    final int next = widget.callback?.call(value) ?? value; /* expect: omit-local-variable-types */
    final int asserted = widget.callback!(value); /* expect: omit-local-variable-types */
    print([index, explicit, next, asserted]);
  }
}
void localCallbacks(int Function(int)? callback, int value) {
  final int? optional = callback?.call(value); /* expect: omit-local-variable-types */
  final int next = callback?.call(value) ?? value; /* expect: omit-local-variable-types */
  final int asserted = callback!(value); /* expect: omit-local-variable-types */
  final int closure = ((int argument) => argument)(value); /* expect: omit-local-variable-types */
  int local(int argument) => argument;
  final int directLocal = local(value); /* expect: omit-local-variable-types */
  print([asserted, optional, next, closure, directLocal]);
}
abstract class SuperCallbackConsumer extends CallbackState<CallbackWidget> {
  void use(String key) {
    final int? index = super.widget.findChildIndexCallback!(key); /* expect: omit-local-variable-types */
    print(index);
  }
}
