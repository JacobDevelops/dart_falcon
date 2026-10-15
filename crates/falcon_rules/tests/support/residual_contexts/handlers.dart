class Trace {}
void optional(Object error, [Trace? trace]) {}
void required(Object error, Trace trace) {}
void take(Function? handler) {}
void typed(void Function(Object, Trace) handler) {}
void check(bool condition) {
  // Different optional positional shapes have general Function as their UP.
  take(condition ? optional : (Object e, Trace t) {});
  take(condition ? (Object e, Trace t) {} : optional);
  final handler = condition ? optional : (Object e, Trace t) {};
  take(condition ? optional : (Object e, Trace t) async {});

  // Equal shapes retain their inferred function context.
  take(condition ? required : (
    Object e, /* expect: avoid-types-on-closure-parameters */
    Trace t /* expect: avoid-types-on-closure-parameters */
  ) {});
  // A concrete downward context remains usable across different shapes.
  typed(condition ? optional : (
    Object e, /* expect: avoid-types-on-closure-parameters */
    Trace t /* expect: avoid-types-on-closure-parameters */
  ) {});
}
