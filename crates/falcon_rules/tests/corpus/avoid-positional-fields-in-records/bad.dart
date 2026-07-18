class RecordExamples {
  /// Positional fields in record literal
  void example1() {
    final record = (1, 'hello'); /* expect: avoid-positional-fields-in-records */
    print(record);
  }

  /// Positional fields in record type return
  (int, String) getInfo() { /* expect: avoid-positional-fields-in-records */
    return (42, 'answer');
  }

  /// Positional fields in record type parameter
  void processRecord((int, String) data) { /* expect: avoid-positional-fields-in-records */
    print(data);
  }

  /// Positional fields in record type variable
  void example2() {
    (String, bool, int) tuple = ('test', true, 5); /* expect: avoid-positional-fields-in-records */
    print(tuple);
  }

  /// Positional fields in record literal with multiple
  void example3() {
    final result = (1, 2, 3, 'four'); /* expect: avoid-positional-fields-in-records */
    print(result);
  }
}
