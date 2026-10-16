class GenericState<T> {
  T get state => throw 0;
  set state(T value) {}
  late T field;
  int count = 0;

  T ownTypeParameter(T value) {
    field = value;
    return field;
  }
}

int substitutedAccessors(GenericState<int> notifier) {
  notifier.state++;
  return notifier.state;
}

int substitutedField(GenericState<int> notifier) {
  notifier.field = 1;
  return notifier.field;
}

int unsubstitutedField(GenericState<int> notifier) {
  notifier.count++; /* expect: join-return-with-assignment */
  return notifier.count;
}
