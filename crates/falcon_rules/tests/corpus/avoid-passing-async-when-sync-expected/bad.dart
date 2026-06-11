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

// More violations
void performAction(void Function(String, int) callback) {
  callback('test', 42);
}

void badAction() {
  performAction((name, count) async { /* expect: avoid-passing-async-when-sync-expected */
    await Future.delayed(Duration(milliseconds: 100));
    print('$name: $count');
  });
}

class DataLoader {
  void loadData(String Function() fetcher) {
    final data = fetcher();
    print(data);
  }

  void loadBad() {
    loadData(() async { /* expect: avoid-passing-async-when-sync-expected */
      await Future.delayed(Duration(seconds: 1));
      return 'data';
    });
  }
}
