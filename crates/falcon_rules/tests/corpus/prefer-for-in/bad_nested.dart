void nested(List<int> items, List<int> other) {
  for (var i = 0; i < items.length; i++) { /* expect: prefer-for-in */
    for (var j = 0; j < other.length; j++) { /* expect: prefer-for-in */
      print(items[i] + other[j]);
    }
  }
}
