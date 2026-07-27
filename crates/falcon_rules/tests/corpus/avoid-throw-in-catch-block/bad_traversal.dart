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
