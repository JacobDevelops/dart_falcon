void check(int value) {
  switch (value) {
    case 0:
      late String result; /* expect: avoid-late-keyword */
      result = 'zero';
      print(result);
  }
}

extension type Wrapper(int value) {
  static late int shared; /* expect: avoid-late-keyword */
}
