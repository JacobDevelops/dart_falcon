void f() {}

Future<void> g() async {}

// Local and top-level variable types.
Null topLevel;

void locals(Object? o) {
  Null local;
  print(local);

  // Casts and cast patterns.
  final n = o as Null;
  print(n);
  if (o case final _ as Null) {}

  // Empty collection literals.
  final list = <Null>[];
  final map = <Null, Null>{};
  final set = <Null>{};
  print([list, map, set]);
}

// Types written directly in a function type.
Null Function(Null) callback = (_) => null;

// extension on Null.
extension on Null {
  bool get isNothing => true;
}

// Extension type representation.
extension type Nothing(Null value) {}

// Overriding a member that does not return void.
class Base {
  Object? value() => 1;
}

class Sub extends Base {
  @override
  Null value() => null;
}

// Unresolvable inheritance stays silent.
class Remote extends SomeUnknownBase {
  Null fetch() => null;
}

// Overriding an SDK member.
class Failure extends Error {
  @override
  Null get stackTrace => null;
}
