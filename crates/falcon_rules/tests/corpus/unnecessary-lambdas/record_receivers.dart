class Handler {
  void consume(int value) {}
}

void recordReceivers() {
  final record = (Handler(), handler: Handler(), callback: print);
  final positional = (int value) => record.$1.consume(value); /* expect: unnecessary-lambdas */
  final named = (int value) => record.handler.consume(value); /* expect: unnecessary-lambdas */
  final callback = (Object? value) => record.callback(value); /* expect: unnecessary-lambdas */
  var mutable = (Handler(),);
  final unstable = (int value) => mutable.$1.consume(value);
  mutable = (Handler(),);
  positional(1);
  named(1);
  callback(1);
  unstable(1);
}
