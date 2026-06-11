// Bad: switch with 11 cases in different context
void processDay(int dayOfWeek) {
  switch (dayOfWeek) { /* expect: max_switch_cases */
    case 0:
      print('Monday');
      break;
    case 1:
      print('Tuesday');
      break;
    case 2:
      print('Wednesday');
      break;
    case 3:
      print('Thursday');
      break;
    case 4:
      print('Friday');
      break;
    case 5:
      print('Saturday');
      break;
    case 6:
      print('Sunday');
      break;
    case 7:
      print('Extra day 1');
      break;
    case 8:
      print('Extra day 2');
      break;
    case 9:
      print('Extra day 3');
      break;
    case 10:
      print('Extra day 4');
      break;
    default:
      print('Unknown day');
  }
}
