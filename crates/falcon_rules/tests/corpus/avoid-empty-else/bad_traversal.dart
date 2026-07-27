void check(bool condition) {
  final callbacks = [
    () {
      if (condition) {
        print('yes');
      } else; /* expect: avoid-empty-else */
    },
  ];
  print(callbacks);
}
