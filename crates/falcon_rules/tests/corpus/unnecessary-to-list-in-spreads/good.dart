void good() {
  var a = [...items];
  var b = [1, ...more];
  var c = <int>{...values};
  var d = [...?maybe];
  var e = [...items.map((e) => e * 2)];
  var f = [items.toList()];
}
