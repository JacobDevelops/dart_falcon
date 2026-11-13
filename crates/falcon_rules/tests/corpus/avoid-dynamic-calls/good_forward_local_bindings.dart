dynamic shared;

void check(int choice) {
  try {
    shared();
    Object shared() => 1;
  } finally {
    shared.toString();
    Object shared = 2;
  }
  switch (choice) {
    case 0:
      shared();
      Object shared() => 3;
      break;
    default:
      shared.toString();
      Object shared = 4;
  }
}
