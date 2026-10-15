// @dart = 3.13

void local(int input) {
  final int value = input; /* expect: unnecessary-final */
  print(value);
}
