extension Checked on Object {
  void run() {
    try {
      print(this);
    } catch (error) {
      failure: {
        throw StateError('$error'); /* expect: avoid-throw-in-catch-block */
      }
      try {
        throw StateError('nested try'); /* expect: avoid-throw-in-catch-block */
      } catch (nested) {
        throw StateError('$nested'); /* expect: avoid-throw-in-catch-block */
      } finally {
        throw StateError('nested finally'); /* expect: avoid-throw-in-catch-block */
      }
    }
  }
}

void delayedCatchScopes() {
  try {
    work();
  } catch (outer) {
    void delayed() {
      throw StateError('delayed local');
      try {
        work();
      } catch (inner) {
        throw StateError('local catch'); /* expect: avoid-throw-in-catch-block */
      }
    }
    final callback = () {
      throw StateError('delayed closure');
      try {
        work();
      } catch (inner) {
        throw StateError('closure catch'); /* expect: avoid-throw-in-catch-block */
      }
    };
    throw StateError('outer restored'); /* expect: avoid-throw-in-catch-block */
  }
}
