void use(Object o) {}

bool matches(String pattern) => pattern.isNotEmpty;

void main() {
  var name = 'Ann';
  use('This string will be concatenated '
      'with the next one.');
  use('Hello' ' world');
  use('nospace' 'here');
  use('Name is ' '$name.');
  use('Name is' '${name}x');
  use('Name is' '$name.');
  use('Name is $name' 'x');
  use(RegExp('(foo bar)' '(baz)'));
  use(new RegExp('a b' 'c'));
  use(matches('a b' 'c'));
  use('tab\t' 'x');
}
