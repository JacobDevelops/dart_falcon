class Base<T> {
  T echo(T value) => value;
}

mixin Echo<T> on Base<T> {
  @override
  T echo(T value) => super.echo(value);
}

class Example extends Base<bool> with Echo {}

void check(Example value) {
  if (value.echo(true) == true) {} /* expect: no-literal-bool-comparisons */
}
