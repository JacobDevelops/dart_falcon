void check() {
  try {
    print('work');
  } catch (error) {
    final callbacks = [
      () {
        throw StateError('$error');
      },
    ];
    void delayed() {
      throw StateError('$error');
    }
    print([callbacks, delayed]);
  }
}
