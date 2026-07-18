void good() {
  var a = {for (final e in list) e: e * 2};
  var b = Map.from(other);
  var c = Map.of(existing);
  var d = Map.fromEntries(entries);
  var e = Map.fromIterables(keys, values);
  var f = Map.fromIterable(list);
}
