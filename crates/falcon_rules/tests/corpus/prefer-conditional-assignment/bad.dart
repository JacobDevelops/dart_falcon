void f(int? a, C obj) {
  if (a == null) { a = 0; } /* expect: prefer-conditional-assignment */
  if (a == null) a = 1; /* expect: prefer-conditional-assignment */
  if (null == a) { a = 2; } /* expect: prefer-conditional-assignment */
  if (obj.field == null) { obj.field = 3; } /* expect: prefer-conditional-assignment */
  if (obj.field == null) obj.field = 4; /* expect: prefer-conditional-assignment */
  print('$a ${obj.field}');
}

class C {
  int? field;
}

class StateBase {
  int? get state => null;
  set state(int? value) {}
}

class InheritedState extends StateBase {}

void pairedAccessors(InheritedState notifier, List<InheritedState> notifiers, int? value) {
  if (notifier.state == null) notifier.state = 1; /* expect: prefer-conditional-assignment */
  if (notifiers.first.state == null) notifiers.first.state = 1; /* expect: prefer-conditional-assignment */
  if (value == null) { { value = 1; } } /* expect: prefer-conditional-assignment */
  if (value == null) value ??= 1; /* expect: prefer-conditional-assignment */
}

void unresolvedMembers(dynamic receiver) {
  // Unresolved dynamic properties have the same null canonical element in Dart.
  if (receiver.first == null) receiver.second = 1; /* expect: prefer-conditional-assignment */
}
