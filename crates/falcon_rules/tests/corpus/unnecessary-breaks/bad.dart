void f(int x) {
  switch (x) {
    case 1:
      print('one');
      break; /* expect: unnecessary-breaks */
    case 2:
    case 3:
      print('two or three');
      break; /* expect: unnecessary-breaks */
    default:
      print('other');
      break; /* expect: unnecessary-breaks */
  }
  for (var i = 0; i < x; i++) {
    switch (i) {
      case 0:
        print(i);
        print(i + 1);
        break; /* expect: unnecessary-breaks */
      case 1:
        if (x > 2) {
          print(x);
        }
        break; /* expect: unnecessary-breaks */
    }
  }
}
