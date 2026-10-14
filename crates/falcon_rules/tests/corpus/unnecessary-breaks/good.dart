void f(int x) {
  switch (x) {
    case 1:
      break;
    case 2:
      print('two');
    default:
      print('other');
  }
  outer:
  for (var i = 0; i < x; i++) {
    switch (i) {
      case 0:
        print(i);
        break outer;
      case 1:
        {
          print(i);
          break;
        }
      default:
        print('x');
    }
  }
  while (x > 0) {
    x--;
    if (x == 3) break;
  }
}
