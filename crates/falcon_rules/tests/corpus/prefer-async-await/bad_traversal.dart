void chained(dynamic future, int value) {
  final result = switch (value) {
    0 => future.then((item) => item), /* expect: prefer-async-await */
    _ => (
      future,
      [future.then((item) => item)], /* expect: prefer-async-await */
    ),
  };
  print(result);
}

extension Values on int {
  void call(dynamic future) {
    final result = {
      'value': future.then((item) => item), /* expect: prefer-async-await */
    };
    print(result);
  }
}
