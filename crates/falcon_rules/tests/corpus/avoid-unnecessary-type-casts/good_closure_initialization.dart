// Explicit final types do not promote to their initializer's type.
void explicitlyTypedFinal() {
  final String? name = 'x';
  print(name as String);
}

// Captured writes prohibit promotion even before the writing closure is visited.
void laterWritingClosure(int? value, List<Function> callbacks) {
  callbacks.add(() {
    if (value != null) print(value as int);
  });
  callbacks.add(() => value = null);
}

void selfWritingClosure(int? value, List<Function> callbacks) {
  callbacks.add(() {
    if (value != null) print(value as int);
    value = null;
  });
}
