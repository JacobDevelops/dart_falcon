// Good: all case labels are unique
void processCode(int code) {
  switch (code) {
    case 1:
      print('first');
      break;
    case 2:
      print('second');
      break;
    case 3:
      print('third');
      break;
  }
}

// Good: unique string cases
void processStatus(String status) {
  switch (status) {
    case 'active':
      print('Active');
      break;
    case 'inactive':
      print('Inactive');
      break;
    case 'pending':
      print('Pending');
      break;
  }
}
