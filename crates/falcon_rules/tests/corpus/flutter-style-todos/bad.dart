/* expect: flutter-style-todos */ // TODO: implement
/* expect: flutter-style-todos */ // todo(user) missing colon
/* expect: flutter-style-todos */ /// TODO(user): doc comments are not to-dos
/* expect: flutter-style-todos */ //TODO implement
/* expect: flutter-style-todos */ // TODO(): empty name
/* expect: flutter-style-todos */ // ToDo(user):missing space
void main() {
  /* expect: flutter-style-todos */ // TODO(-user): name must start alphanumeric
}

int a = 1;
var s = '${a /* expect: flutter-style-todos */ // TODO wrong
}';
