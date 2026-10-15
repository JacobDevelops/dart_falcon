void pair({int first = 1, int second = 2}) {}
void positional(int first, [int second = 1]) {}
void requiredPos(int value) {}
void requiredNamed({required int value}) {}
class NamedMaker { NamedMaker({int value = 1}); }
class RequiredNamedMaker { RequiredNamedMaker.named(int? value); }
class OptionalBase { OptionalBase([int? value]); }
class RequiredChild extends OptionalBase { RequiredChild(super.value); }

void notForwarding() {
  final renamed = ({int value = 1}) => pair(first: value);
  final swapped = ({int first = 1, int second = 2}) => pair(first: second, second: first);
  final positionalSwap = (int first, [int second = 1]) => positional(second, first);
  final duplicate = ({int first = 1, int second = 2}) => pair(first: first, second: first);
  final missing = ({int first = 1, int second = 2}) => pair(first: first);
  final discarded = ({int first = 1, int second = 2}) { pair(first: first, second: second); };
  final requiredPosition = ([int value = 1]) => requiredPos(value);
  final requiredName = ({int value = 1}) => requiredNamed(value: value);
  // The analyzer excludes constructor calls with named arguments.
  final maker = ({int value = 1}) => NamedMaker(value: value);
  final explicitMaker = ({int value = 1}) => new NamedMaker(value: value);
  final positionalMaker = (int value) => NamedMaker(value: value);
  final requiredNamedConstructor = ([int? value]) => new RequiredNamedMaker.named(value);
  final requiredSuper = ([int? value]) => RequiredChild(value);
  final explicitRequiredSuper = ([int? value]) => new RequiredChild(value);
  void local(int value) {}
  final localRequired = ([int value = 1]) => local(value);
}
