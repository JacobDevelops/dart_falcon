String forceString(String? value) => value!;


void concretePattern(String? value) {
  if (value case var item!) {
    print(item);
  }
}

class T {}

T classNamedT(T? value) => value!;

T outerReturnDoesNotLeak<T>(T? value) {
  final int Function() callback = () => value!;
  callback();
  return value;
}

T namedLocalDoesNotInheritContext<T>(T? value) {
  int local() => value!;
  local();
  return value;
}

T bounded<T extends Object>(T? value) => value!;
class Generic<T> {
  T force(T? value) => value!;
}
void inferredClosure<T>(T? value) {
  final T Function(T?) callback = (T? inner) => inner!;
  final T Function<T>(T?) generic = <T>(T? inner) => inner!;
  callback(value);
  generic<T>(value);
}

T checkedAssignment<T>(T? value) {
  value!;
  T result;
  result = value!;
  return result;
}

void selfAssignment<T>(T? value) {
  value = value!;
}
