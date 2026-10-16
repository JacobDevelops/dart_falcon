// Function type tests promote general Function values without making unknown
// callbacks safe. A nullable fallback inherits the callback's parameter types.
void dispatch(Function callback, Object error, StackTrace trace) {
  if (callback is void Function(Object, StackTrace)) {
    callback(error, trace);
  } else if (callback is void Function(Object)) {
    callback(error);
  }
}

class Context {
  void read() {}
}

typedef Callback = void Function(Context);

void fallback(Callback? callback) {
  final executor = callback ?? (ctx) => ctx.read();
  executor(Context());
}

void unresolved(dynamic callback) {
  callback(); /* expect: avoid-dynamic-calls */
}

extension ImplicitCallbackCall on Callback {
  void implicit(Context value) {
    call(value); /* expect: avoid-dynamic-calls */
  }

  void explicit(Context value) {
    this.call(value);
  }
}
