void checks(List<int> xs, Set<int> ys) {
  var a = [1]..addAll(xs);
  var b = xs..addAll([2]);
  var c = xs..add(2);
  var d = {1}..add(2);
  var e = {1}..addAll({2});
  var f = {1}..addAll(ys);
}
