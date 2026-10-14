void use(Object o) {}

void main() {
  var name = 'Ann';
  /* expect: leading-newlines-in-multiline-strings */ use('''{
  "a": 1
}''');
  /* expect: leading-newlines-in-multiline-strings */ use("""first line
second line""");
  /* expect: leading-newlines-in-multiline-strings */ use(r'''raw first
raw second''');
  /* expect: leading-newlines-in-multiline-strings */ use('''$name
interpolated''');
  /* expect: leading-newlines-in-multiline-strings */ use('a' '''b
adjacent part''');
}
