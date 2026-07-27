void check(bool condition) {
  label: {} /* expect: no-empty-block */

  final callbacks = <void Function()>[
    () {}, /* expect: no-empty-block */
  ];
  print(callbacks);
}
