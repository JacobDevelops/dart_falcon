class RecordExamples {
  /// Positional fields in record literal
  void example1() {
    final record = (1, 'hello');
    print(record);
  }

  /// Positional fields in record type return
  (int, String) getInfo() {
    return (42, 'answer');
  }

  /// Positional fields in record type parameter
  void processRecord((int, String) data) {
    print(data);
  }

  /// Positional fields in record type variable
  void example2() {
    (String, bool, int) tuple = ('test', true, 5);
    print(tuple);
  }

  /// Positional fields in record literal with multiple
  void example3() {
    final result = (1, 2, 3, 'four');
    print(result);
  }
}

// Declarations inside nested containers are also allowed.
void containersRegression() {
  lbl: {
    final inLabeled = (1, 2);
    print(inLabeled);
  }
  final closure = () {
    final inClosure = (3, 4);
    return inClosure;
  };
  print(closure);
}

typedef Pair = (int, String);

void typePositions(Object value) {
  final cast = value as (int, String);
  final test = value is (int, String);
  final callback = ((int, String) pair) => pair;
  print([cast, test, callback]);
}

final callback = () {
  final pair = (1, 2);
  return pair;
};
