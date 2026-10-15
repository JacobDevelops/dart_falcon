// Function-expression invocations are not method invocations for this lint.
typedef Sink = void Function(void);
void discard(void value) {}
Sink global = discard;
Sink get getter => discard;
class Callbacks {
  Sink field = discard;
  Sink get getter => discard;
  void method(void value) {}
  void inside(Object event) {
    field(event);
    getter(event);
    method(event); /* expect: void-checks */
  }
}
void invoke(Sink callback, Object event, Callbacks callbacks) {
  callback(event);
  (callback)(event);
  callback.call(event); /* expect: void-checks */
  discard(event); /* expect: void-checks */
  (discard)(event);
  global(event);
  getter(event);
  callbacks.field(event);
  callbacks.getter(event);
  callbacks.method(event); /* expect: void-checks */
  callbacks..field(event)..getter(event);
  callbacks
    ..field.call(event) /* expect: void-checks */
    ..getter.call(event); /* expect: void-checks */
  final inferred = discard;
  inferred(event);
  void local(void value) {}
  local(event); /* expect: void-checks */
  (local)(event);
}
