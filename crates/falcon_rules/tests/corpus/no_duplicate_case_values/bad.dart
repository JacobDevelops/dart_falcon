// Bad: switch with duplicate case values
void processCode(int code) {
  switch (code) {
    case 1:
      print('first');
      break;
    case 1: /* expect: no_duplicate_case_values */
      print('duplicate');
      break;
    case 2:
      print('second');
      break;
  }
}

// Bad: duplicate string cases
void processStatus(String status) {
  switch (status) {
    case 'active':
      print('Active');
      break;
    case 'active': /* expect: no_duplicate_case_values */
      print('Also active');
      break;
    case 'inactive':
      print('Inactive');
      break;
  }
}
