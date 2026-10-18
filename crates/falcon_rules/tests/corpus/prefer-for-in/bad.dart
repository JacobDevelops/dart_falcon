void canonical(List<int> items) {
  for (var i = 0; i < items.length; i++) { /* expect: prefer-for-in */
    print(items[i]);
  }
  for (int index = 0; index < items.length; ++index) { /* expect: prefer-for-in */
    print(items[index]);
  }
  for (var i = 0; i < items.length; i += 1) { /* expect: prefer-for-in */
    print('$i: ${items[i]}');
  }
  for (var i = (0); (i) < (items.length); i++) /* expect: prefer-for-in */
    print(items[i]);
  for (var i = 0; i < items.length; i++) { /* expect: prefer-for-in */
    if (items[i] < 0) continue;
    if (items[i] == 10) break;
    print(i);
  }
}

void literals() {
  final values = [1, 2, 3];
  for (var j = 0; j < values.length; j++) { /* expect: prefer-for-in */
    print(values[j]);
  }
  List<String> names = ['Ada', 'Grace'];
  for (var n = 0; n < names.length; n += 1) { /* expect: prefer-for-in */
    print(names[n]);
  }
}
