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

// Good: passing sync callbacks to sync parameters
void performAction(void Function(String, int) callback) {
  callback('test', 42);
}

void goodAction() {
  performAction((name, count) {
    print('$name: $count');
  });
}

// Good: using named function for sync callback
class DataLoader {
  String _fetchData() {
    return 'data';
  }

  void loadData(String Function() fetcher) {
    final data = fetcher();
    print(data);
  }

  void loadGood() {
    loadData(_fetchData);
  }
}

// Good: sync arrow function
void processItems(void Function(String) handler) {
  handler('item');
}

void goodProcess() {
  processItems((item) => print('Processing: $item'));
}
