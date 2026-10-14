void f(bool a, bool b, bool? maybe, int i, dynamic d) {
  final x = a || b;
  final y = a ? b : !b;
  final z = a ? maybe : true;
  final w = a ? true : maybe;
  final v = a ? 1 : 2;
  final u = a ? d : false;
  final t = a ? 'yes' : 'no';
  print([x, y, z, w, v, u, t]);
}
