void syncCallback(void Function() callback) => callback();
void asyncCallback(Future<void> Function() callback) {}
void _privateCallback(void Function() callback) {}

class SyncApi {
  void run(void Function() callback) => callback();
}

class AsyncApi {
  void run(Future<void> Function() callback) {}
}
