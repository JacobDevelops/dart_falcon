// Bad: an 11-case switch under a labeled block must still be checked.
void handleLabeled(int x) {
  outer: {
    switch (x) { /* expect: max-switch-cases */
      case 1:
        print('one');
        break;
      case 2:
        print('two');
        break;
      case 3:
        print('three');
        break;
      case 4:
        print('four');
        break;
      case 5:
        print('five');
        break;
      case 6:
        print('six');
        break;
      case 7:
        print('seven');
        break;
      case 8:
        print('eight');
        break;
      case 9:
        print('nine');
        break;
      case 10:
        print('ten');
        break;
      case 11:
        print('eleven');
        break;
      default:
        print('other');
    }
  }
}
