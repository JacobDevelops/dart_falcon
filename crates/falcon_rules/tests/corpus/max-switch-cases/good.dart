// Good: switch with 9 cases within limit
void handleValue(int x) {
  switch (x) {
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
    default:
      print('other');
  }
}

// Good: switch with 10 cases (at the limit)
void handleStatus(int code) {
  switch (code) {
    case 200:
      print('ok');
      break;
    case 201:
      print('created');
      break;
    case 204:
      print('no content');
      break;
    case 301:
      print('moved permanently');
      break;
    case 304:
      print('not modified');
      break;
    case 400:
      print('bad request');
      break;
    case 401:
      print('unauthorized');
      break;
    case 403:
      print('forbidden');
      break;
    case 404:
      print('not found');
      break;
    default:
      print('other');
  }
}

// Good: switch with 5 cases well under limit
void processType(String type) {
  switch (type) {
    case 'int':
      print('integer');
      break;
    case 'double':
      print('floating point');
      break;
    case 'string':
      print('text');
      break;
    case 'bool':
      print('boolean');
      break;
    default:
      print('unknown type');
  }
}
