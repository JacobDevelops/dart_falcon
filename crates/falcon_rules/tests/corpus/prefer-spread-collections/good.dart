void good() {
  final other = [3, 4];
  final first = [1];
  final second = [2];
  final someList = <int>[];
  var a = [1, 2, ...other];
  var b = <int>[...first, ...second];
  someList.addAll([3, 4]);
  var c = someList..sort();
  var d = someList..addAll([1]);
}
