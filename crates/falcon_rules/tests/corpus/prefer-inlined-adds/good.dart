void good() {
  final items = <int>[];
  var a = [1, 2, 3];
  items.add(4);
  var b = items..add(5);
  var c = [1]..addAll([2, 3]);
  var d = [1, 2]..sort();
}
