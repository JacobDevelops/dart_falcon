// Bad: switch with 11 cases exceeds max (default: 10)
void handleValue(int x) {
  switch (x) { /* expect: max_switch_cases */
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
