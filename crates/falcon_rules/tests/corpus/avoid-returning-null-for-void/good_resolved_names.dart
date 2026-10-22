class Future<T> {}

Future<void> localFuture() async => null;

class LocalFutureMembers {
  Future<void> method() async {
    return null;
  }

  Future<void> get value async => null;
}

void nestedLocalFuture() {
  Future<void> inner() async => null;
  inner();
}

void genericFuture<Future>() {
  Future inner() async => null;
  inner();
}
