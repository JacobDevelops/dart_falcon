void f(int? a, int? b, C obj) {
  if (a == null) {
    a = 0;
  } else {
    a = 1;
  }
  if (a == null) {
    b = 2;
  }
  if (a != null) {
    a = 3;
  }
  if (a == null) {
    print(a);
  }
  if (a == null) {
    a = 0;
    b = 1;
  }
  a ??= 5;
  print('$a $b ${obj.field}');
}

class C {
  int? field;
}

class StateBase {
  int? get state => null;
  set state(int? value) {}
}

class ReadOverride extends StateBase {
  @override
  int? get state => super.state;
}

void distinctTargets(
  ReadOverride notifier,
  StateBase first,
  StateBase second,
  List<int?> values,
  Map<String, int?> entries,
) {
  if (notifier.state == null) notifier.state = 1;
  if (first.state == null) second.state = 1;
  if (values[0] == null) values[0] = 1;
  if (entries['key'] == null) entries['key'] = 1;
}

void differentAccessSyntax(StateBase notifier) {
  if (notifier.state == null) (notifier).state = 1;
  if (notifier.state == null) (notifier.state = 1);
}
