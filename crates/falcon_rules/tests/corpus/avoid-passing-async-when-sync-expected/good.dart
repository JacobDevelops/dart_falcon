// Good: passing sync function to sync parameter
void processSync(void Function() callback) {
  callback();
}

void example() {
  processSync(() {
    print('Done');
  });
}

class Handler {
  void setup(void Function(String) handler) {
    handler('test');
  }

  void goodSetup() {
    setup((value) {
      processSync(value);
    });
  }
}

void executeCallback(String Function(int) fn) {
  print(fn(42));
}

void goodUse() {
  executeCallback((n) {
    return '$n';
  });
}

// OK: async callback passed to async parameter
void processAsync(Future<void> Function() callback) async {
  await callback();
}

void asyncToAsync() {
  processAsync(() async {
    await Future.delayed(Duration(seconds: 1));
    print('Done');
  });
}

// OK: extracting async logic separately
void separated(void Function() callback) {
  callback();
}

void _asyncWork() async {
  await Future.delayed(Duration(seconds: 1));
}

void extract() {
  separated(() {
    _asyncWork(); // fire and forget (or capture result)
  });
}
