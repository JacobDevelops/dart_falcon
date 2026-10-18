class IndexedOnly {
  int get length => 3;
  int operator [](int index) => index;
}

void typeParameter<List extends IndexedOnly>(List items) {
  for (var i = 0; i < items.length; i++) {
    print(items[i]);
  }
}
