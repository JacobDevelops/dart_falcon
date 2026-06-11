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

// Bad: duplicate boolean cases
void processBool(bool flag) {
  switch (flag) {
    case true:
      print('True branch');
      break;
    case true: /* expect: no_duplicate_case_values */
      print('Duplicate true');
      break;
    case false:
      print('False branch');
      break;
  }
}

// Bad: double duplicate cases
void processDouble(double value) {
  switch (value) {
    case 1.5:
      print('One point five');
      break;
    case 2.5:
      print('Two point five');
      break;
    case 1.5: /* expect: no_duplicate_case_values */
      print('Duplicate 1.5');
      break;
    case 2.5: /* expect: no_duplicate_case_values */
      print('Duplicate 2.5');
      break;
  }
}
