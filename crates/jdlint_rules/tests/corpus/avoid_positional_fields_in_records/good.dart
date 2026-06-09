class RecordExamples {
  /// Named fields in record literal
  void example1() {
    final record = (id: 1, name: 'hello');
    print(record);
  }

  /// Named fields in record type return
  ({int id, String name}) getInfo() {
    return (id: 42, name: 'answer');
  }

  /// Named fields in record type parameter
  void processRecord(({int id, String name}) data) {
    print(data);
  }

  /// Named fields in record type variable
  void example2() {
    ({String name, bool active, int count}) tuple = (name: 'test', active: true, count: 5);
    print(tuple);
  }

  /// Named fields in record literal with multiple
  void example3() {
    final result = (a: 1, b: 2, c: 3, d: 'four');
    print(result);
  }

  /// Record with mixed named fields
  void example4() {
    final data = (userId: 100, isActive: true);
    print(data.userId);
    print(data.isActive);
  }
}
