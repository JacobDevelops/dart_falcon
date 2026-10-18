void boundaries(List<int> items, List<int> other) {
  for (final item in items) {
    print(item);
  }
  for (var i = 1; i < items.length; i++) {
    print(items[i]);
  }
  for (var i = 0; items.length > i; i++) {
    print(items[i]);
  }
  for (var i = 0; i <= items.length; i++) {
    print(i);
  }
  for (var i = 0; i < items.length; i += 2) {
    print(items[i]);
  }
  for (var i = 0; i < items.length; i--) {
    print(items[i]);
    break;
  }
  for (var i = 0; i < items.length; i++) {
    i += 1;
    print(items[i]);
  }
  for (var i = 0; i < items.length; i++) {
    items.add(i);
  }
  for (var i = 0; i < items.length; i++) {
    items[i] = 0;
  }
  for (var i = 0; i < items.length; i++) {
    items = other;
  }
  for (var i = 0; i < items.length; i++) {
    print(items[i + 1]);
  }
  for (var i = 0; i < items.length; i++) {
    print(items);
  }
  for (var i = 0, j = 0; i < items.length; i++, j++) {
    print(items[i]);
  }
  var index = 0;
  for (index = 0; index < items.length; index++) {
    print(items[index]);
  }
  for (var i = 0; i < items.length; i++) {
    final callback = () => items[i];
    print(callback);
  }
  for (var i = 0; i < items.length; i++) {
    final alias = items;
    alias.clear();
  }
  for (var i = 0; i < items.length; i++) {
    final i = 1;
    print(items[i]);
  }
  for (var i = 0; i < items.length; i++) {
    (i, index) = (1, 2);
  }
}

void nonLists(String text, Map<int, int> entries, dynamic unknown) {
  for (var i = 0; i < text.length; i++) {
    print(text[i]);
  }
  for (var i = 0; i < entries.length; i++) {
    print(entries[i]);
  }
  for (var i = 0; i < unknown.length; i++) {
    print(unknown[i]);
  }
}

class Owner {
  List<int> items = [1, 2];
  void fields() {
    for (var i = 0; i < items.length; i++) {
      print(items[i]);
    }
    for (var i = 0; i < this.items.length; i++) {
      print(this.items[i]);
    }
  }
}

Future<void> suspension(List<int> items) async {
  for (var i = 0; i < items.length; i++) {
    await Future<void>.value();
    print(items[i]);
  }
}
