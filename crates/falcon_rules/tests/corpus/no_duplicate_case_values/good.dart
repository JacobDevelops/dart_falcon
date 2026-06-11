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

// Good: boolean cases without duplication
void handleBool(bool value) {
  switch (value) {
    case true:
      print('True');
      break;
    case false:
      print('False');
      break;
  }
}

// Good: unique double values
void handleDouble(double value) {
  switch (value) {
    case 1.5:
      print('One point five');
      break;
    case 2.5:
      print('Two point five');
      break;
    case 3.5:
      print('Three point five');
      break;
  }
}

// Good: unique negative numbers
void handleNegative(int n) {
  switch (n) {
    case -1:
      print('Negative one');
      break;
    case -2:
      print('Negative two');
      break;
    case 0:
      print('Zero');
      break;
    case 1:
      print('One');
      break;
  }
}
