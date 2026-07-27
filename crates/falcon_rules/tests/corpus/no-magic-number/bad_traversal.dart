extension type Counter(int value) {
  int scaled() {
    return switch (value) {
      _ => (value * 42,), /* expect: no-magic-number */
    }.$1;
  }
}
