class Label {
  final String value;
  const Label(this.value);
}

String render(String value) => value;

void example(Label label, String value) {
  print('value: $label.value'); /* expect: avoid-missing-interpolation, msg: "Interpolation leaves a member access or call as literal text; wrap the complete expression in `${...}`." */
  print("empty: $label.value.isEmpty"); /* expect: avoid-missing-interpolation */
  print('$render(value)'); /* expect: avoid-missing-interpolation */
  print('rendered: $render()'); /* expect: avoid-missing-interpolation */
  print('''$label.value'''); /* expect: avoid-missing-interpolation */
  print('prefix ' '$label.value'); /* expect: avoid-missing-interpolation */
  print('$value.length and $label.value'); /* expect: avoid-missing-interpolation */
  print('backslash: \\ $label.value'); /* expect: avoid-missing-interpolation */
  print("${'$label.value'}"); /* expect: avoid-missing-interpolation */
  print('''prefix
$label.value'''); /* expect: avoid-missing-interpolation */
  print('value'); /* expect: avoid-missing-interpolation */
}

void localNames(String parameter) {
  print('parameter'); /* expect: avoid-missing-interpolation */
  final String title = 'Heading';
  print('title'); /* expect: avoid-missing-interpolation */
  String mutable = 'Current';
  print("mutable"); /* expect: avoid-missing-interpolation */
  const String constant = 'Fixed';
  print('constant'); /* expect: avoid-missing-interpolation */
  {
    final String nested = 'Inner';
    print('nested'); /* expect: avoid-missing-interpolation */
  }
}
