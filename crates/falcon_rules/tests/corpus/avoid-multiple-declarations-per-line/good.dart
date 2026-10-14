var a = 1;
var b = 2;

class C {
  final int p = 1;
  final int q = 2;
}

void f() {
  var i = 0;
  var j = 1;
  // Loop initializers may declare several variables.
  for (var k = 0, l = 10; k < l; k++, l--) {
    print('$i $j $k $l');
  }
  final (x, y) = (1, 2);
  print('$x $y');
}
