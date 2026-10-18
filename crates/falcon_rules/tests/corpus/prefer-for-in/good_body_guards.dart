void shadowing(List<int> items, List<List<int>> groups) {
  for (var i = 0; i < items.length; i++) {
    for (final i in [1, 2]) {
      print(items[i]);
    }
  }
  for (var i = 0; i < items.length; i++) {
    for (final items in groups) {
      print(items[i]);
    }
  }
  for (var i = 0; i < items.length; i++) {
    final (i, value) = (1, 2);
    print(items[i] + value);
  }
  for (var i = 0; i < items.length; i++) {
    try {
      throw 1;
    } catch (i) {
      print(i);
    }
    print(items[i]);
  }
}

void writes(List<int> items) {
  for (var i = 0; i < items.length; i++) {
    items[i]++;
  }
  for (var i = 0; i < items.length; i++) {
    --items[i];
  }
  for (var i = 0; i < items.length; i++) {
    i--;
    print(items[i]);
  }
  for (var i = 0; i < items.length; i++) {
    items..clear();
  }
  for (var i = 0; i < items.length; i++) {
    int read() => items[i];
    print(read());
  }
}

void dynamicRebinding() {
  dynamic values = [1];
  values = 'ab';
  for (var i = 0; i < values.length; i++) {
    print(values[i]);
  }
}

Stream<int> yielding(List<int> items) async* {
  for (var i = 0; i < items.length; i++) {
    yield items[i];
  }
}

Future<void> nestedSuspension(List<int> items, Stream<int> stream) async {
  for (var i = 0; i < items.length; i++) {
    await for (final value in stream) {
      print(items[i] + value);
    }
  }
}
