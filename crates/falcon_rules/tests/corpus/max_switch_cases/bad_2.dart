// Bad: switch with 13 cases
void handleColor(String color) {
  switch (color) { /* expect: max_switch_cases */
    case 'red':
      print('red');
      break;
    case 'green':
      print('green');
      break;
    case 'blue':
      print('blue');
      break;
    case 'yellow':
      print('yellow');
      break;
    case 'cyan':
      print('cyan');
      break;
    case 'magenta':
      print('magenta');
      break;
    case 'black':
      print('black');
      break;
    case 'white':
      print('white');
      break;
    case 'gray':
      print('gray');
      break;
    case 'orange':
      print('orange');
      break;
    case 'purple':
      print('purple');
      break;
    case 'pink':
      print('pink');
      break;
    case 'brown':
      print('brown');
      break;
    default:
      print('unknown color');
  }
}
