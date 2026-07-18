void f(Object x) {
  if (x is! int) {}
  var a = x is String;
  final b = !(x is! double);
  var c = !someBool(x);
  print(x is bool);
  print('$a $b $c');
}

bool someBool(Object x) => true;
