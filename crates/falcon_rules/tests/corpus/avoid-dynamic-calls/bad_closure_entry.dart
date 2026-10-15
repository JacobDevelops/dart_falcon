// Closure entry blocks promotion for writes captured anywhere, including later closures.
void laterWritingClosure(dynamic value, List<Function> callbacks) {
  callbacks.add(() {
    if (value is String) print(value.length); /* expect: avoid-dynamic-calls */
  });
  callbacks.add(() => value = 0);
}

void selfWritingClosure(dynamic value, List<Function> callbacks) {
  callbacks.add(() {
    if (value is String) print(value.length); /* expect: avoid-dynamic-calls */
    value = 0;
  });
}

void laterWritingLocalFunction(dynamic value, List<Function> callbacks) {
  void read() {
    if (value is String) print(value.length); /* expect: avoid-dynamic-calls */
  }
  void write() { value = 0; }
  callbacks.add(read);
  callbacks.add(write);
}

// Direct enclosing writes forget entry promotions but do not prohibit fresh tests.
void directWriteAllowsRetest(dynamic value, List<Function> callbacks) {
  callbacks.add(() {
    if (value is String) print(value.length);
  });
  value = 0;
}

// Synthetic closure-entry captures must not affect the enclosing flow state.
void readOnlyClosureBeforeCapture(dynamic value, List<Function> callbacks) {
  if (value is! String) return;
  callbacks.add(() {
    if (value is String) print(value.length); /* expect: avoid-dynamic-calls */
  });
  print(value.length);
  callbacks.add(() => value = 0);
  if (value is String) print(value.length); /* expect: avoid-dynamic-calls */
}

// An actual nested write still escapes into the enclosing flow state.
void nestedWritingClosure(dynamic value, List<Function> callbacks) {
  if (value is! String) return;
  callbacks.add(() {
    callbacks.add(() => value = 0);
  });
  if (value is String) print(value.length); /* expect: avoid-dynamic-calls */
}
