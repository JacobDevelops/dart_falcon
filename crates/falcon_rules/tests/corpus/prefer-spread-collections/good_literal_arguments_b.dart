void checks(List<int> xs, Set<int> ys) {
  var a = [1]..addAll([2]);
  var b = [1]..add(2);
  var c = xs..addAll([2]);
  var d = xs..addAll(ys);
  var e = {1}..addAll({2});
  var f = {1}..addAll(ys);
  var g = <int, int>{}..addAll({1: 2});
}
