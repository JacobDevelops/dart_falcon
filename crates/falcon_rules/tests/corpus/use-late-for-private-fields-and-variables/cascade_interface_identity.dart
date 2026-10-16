abstract class Derivation {
  Set<int>? _newObservables;
}

class Computed implements Derivation {
  @override
  Set<int>? _newObservables; /* expect: use-late-for-private-fields-and-variables */
}

class Reaction implements Derivation {
  @override
  Set<int>? _newObservables; /* expect: use-late-for-private-fields-and-variables */
}

void reset(Derivation derivation) {
  derivation.._newObservables = null;
  print(derivation._newObservables);
}

class Direct {
  Set<int>? _newObservables;
}

void resetDirect(Direct direct) {
  direct.._newObservables = null;
}
