void select(int value) {
  switch (value) {
    case 0:
      print(0);
      break;

    case 1:
      switch (value) {
        case 1:
          break;

        default:
          break;
      }
      break;

    default:
      break;
  }
}

void single(int value) {
  switch (value) {
    default:
      break;
  }
}

void grouped(int value) {
  switch (value) {
    case 0:
    case 1:
      print(value);
      break;

    // These labels share a body.
    case 2:
    case 3:
      break;

    /* The blank line precedes this comment. */
    default:
      break;
  }
}
