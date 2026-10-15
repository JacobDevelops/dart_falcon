class Payload {
  dynamic value;
  Function callback;
  Payload(this.value, this.callback);
}

void differentBinding(String event) {
  print(event.length);
}

void dynamicBinding(dynamic event) {
  print(event.length); /* expect: avoid-dynamic-calls */
}

void memberTypes(Payload payload) {
  payload.value.handle(); /* expect: avoid-dynamic-calls */
  payload.callback(); /* expect: avoid-dynamic-calls */
}

class Base {
  dynamic data;
}

class Derived extends Base {
  void read() {
    data.handle(); /* expect: avoid-dynamic-calls */
  }
}

void contextFreeClosure() {
  var callback = (value) {
    print(value.length); /* expect: avoid-dynamic-calls */
  };
  callback('text');
}

void acceptsDynamicCallback(void Function(dynamic) callback) {}

void explicitlyDynamicContext() {
  acceptsDynamicCallback((value) {
    print(value.length); /* expect: avoid-dynamic-calls */
  });
}

void acceptsUntypedValue(dynamic callback) {}

void knownNonFunctionContext(dynamic callable, Function function) {
  acceptsUntypedValue((value) {
    print(value.length); /* expect: avoid-dynamic-calls */
  });
  callable((value) { /* expect: avoid-dynamic-calls */
    print(value.length); /* expect: avoid-dynamic-calls */
  });
  function((value) { /* expect: avoid-dynamic-calls */
    print(value.length); /* expect: avoid-dynamic-calls */
  });
}

void dynamicMemberContext(dynamic receiver) {
  receiver.accept((value) { /* expect: avoid-dynamic-calls */
    print(value.length); /* expect: avoid-dynamic-calls */
  });
}
