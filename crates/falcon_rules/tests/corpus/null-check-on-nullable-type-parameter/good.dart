String forceString(String? value) => value!;

T? keepNullable<T>(T? value) => value!;

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
