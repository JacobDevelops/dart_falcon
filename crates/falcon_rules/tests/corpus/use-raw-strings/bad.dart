void use(Object o) {}

void main() {
  use('A string with only \\ and \$'); /* expect: use-raw-strings */
  use('C:\\Users'); /* expect: use-raw-strings */
  use("costs \$5"); /* expect: use-raw-strings */
  use('''multi \\ line'''); /* expect: use-raw-strings */
  use('ok' 'part \\'); /* expect: use-raw-strings */
}
