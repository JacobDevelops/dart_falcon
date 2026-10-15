dynamic config;

void member(dynamic event) {
  print(event.name); /* expect: avoid-dynamic-calls */
  event.handle(); /* expect: avoid-dynamic-calls */
  event?.handle(); /* expect: avoid-dynamic-calls */
}

void operators(dynamic a) {
  print(a + 1); /* expect: avoid-dynamic-calls */
  print(a < 2); /* expect: avoid-dynamic-calls */
  print(-a); /* expect: avoid-dynamic-calls */
  a++; /* expect: avoid-dynamic-calls */
  a += 1; /* expect: avoid-dynamic-calls */
  print(a[0]); /* expect: avoid-dynamic-calls */
}

void invocation(dynamic callback, Function handler) {
  callback(); /* expect: avoid-dynamic-calls */
  handler(1); /* expect: avoid-dynamic-calls */
}

void json(Map<String, dynamic> data, List<dynamic> items) {
  print(data['user'].name); /* expect: avoid-dynamic-calls */
  print(data['user']['id']); /* expect: avoid-dynamic-calls */
  items.first;
  print(items[0].length); /* expect: avoid-dynamic-calls */
}

void chained(dynamic value) {
  dynamic local = value;
  local.first.last; /* expect: avoid-dynamic-calls */ /* expect: avoid-dynamic-calls */
  local..add(1)..add(2); /* expect: avoid-dynamic-calls */
}

void topLevel() {
  config.load(); /* expect: avoid-dynamic-calls */
}

class Holder {
  dynamic payload;

  void read() {
    print(payload.length); /* expect: avoid-dynamic-calls */
  }
}

void promotedInOneFunction(dynamic scopedValue) {
  if (scopedValue is String) {
    print(scopedValue.length);
  }
}

void stillDynamicInAnotherFunction(dynamic scopedValue) {
  print(scopedValue.length); /* expect: avoid-dynamic-calls */
}

void cascadeFunction(Function callback) {
  callback
    ..call() /* expect: avoid-dynamic-calls */
    ..call(); /* expect: avoid-dynamic-calls */
}
