void good() {
  var a = items.whereType<String>();
  var b = list.where((e) => e.isValid);
  var c = things.where((e) => e is! String);
  var d = values.where((e) => e is int && e > 0);
  var e = data.where((a, b) => a is int);
  var f = items.map((e) => e is String);
}
