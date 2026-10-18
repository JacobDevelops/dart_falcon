void expect(Object? actual, Object? matcher) {}
Future<void> expectLater(Object? actual, Object? matcher) async {}
Function expectAsync0(Function body) => body;
Function expectAsync1(Function body) => body;
Function expectAsyncUntil1(Function body, bool Function() done) => body;
Never fail(String message) => throw message;
