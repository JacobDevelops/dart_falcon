// Bad: switch with 11 priority levels
void handlePriority(int priority) {
  switch (priority) { /* expect: max_switch_cases */
    case 1:
      print('Critical');
      break;
    case 2:
      print('High');
      break;
    case 3:
      print('Medium');
      break;
    case 4:
      print('Low');
      break;
    case 5:
      print('Minor');
      break;
    case 6:
      print('Trivial');
      break;
    case 7:
      print('Enhancement');
      break;
    case 8:
      print('Feature');
      break;
    case 9:
      print('Improvement');
      break;
    case 10:
      print('Note');
      break;
    case 11:
      print('Documentation');
      break;
    default:
      print('Unknown priority');
  }
}
