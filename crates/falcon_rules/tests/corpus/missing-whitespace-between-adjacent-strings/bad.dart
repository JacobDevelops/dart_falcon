void use(Object o) {}

bool matches(String pattern) => pattern.isNotEmpty;

void main() {
  var name = 'Ann';
  use('This string will be concatenated' /* expect: missing-whitespace-between-adjacent-strings */
      'with the next one.');
  use('Hello there' 'world'); /* expect: missing-whitespace-between-adjacent-strings */
  use('$name is' 'here'); /* expect: missing-whitespace-between-adjacent-strings */
  use('one two' 'three four' /* expect: missing-whitespace-between-adjacent-strings */ /* expect: missing-whitespace-between-adjacent-strings */
      'five');
  /* expect: missing-whitespace-between-adjacent-strings */ use('line one' '''
line two''');
  use2(text: 'named arg' 'too'); /* expect: missing-whitespace-between-adjacent-strings */
}

void use2({required String text}) {}
