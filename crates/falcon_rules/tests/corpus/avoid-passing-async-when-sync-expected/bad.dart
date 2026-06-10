// Bad: passing async function to sync parameter
void processSync(void Function() callback) {
  callback();
}

void example() {
  processSync(() async { /* expect: avoid-passing-async-when-sync-expected */
    await Future.delayed(Duration(seconds: 1));
    print('Done');
  });
}

class Handler {
  void setup(void Function(String) handler) {
    handler('test');
  }

  void badSetup() {
    setup((value) async { /* expect: avoid-passing-async-when-sync-expected */
      await processAsync(value);
    });
  }
}

void executeCallback(String Function(int) fn) {
  print(fn(42));
}

void misuse() {
  executeCallback((n) async { /* expect: avoid-passing-async-when-sync-expected */
    await delay();
    return '$n';
  });
}
