// `:final name?` infers the getter name through the null-check pattern, so the
// bound variable has the getter's non-null type. No SDK member signatures are
// needed by this fixture.
extension Recovery<T> on Future<T> {
  Future<T> recover(void Function() handler) => this;
}

abstract class Abortable {
  abstract final Future<void>? abortTrigger;
}

void nullCheck(Object request) {
  if (request case Abortable(:final abortTrigger?)) {
    abortTrigger.recover(() {}); /* expect: discarded-futures */
  }
}

void nullAssert(Object request) {
  if (request case Abortable(:final abortTrigger!)) {
    abortTrigger.recover(() {}); /* expect: discarded-futures */
  }
}

void cast(Object request) {
  if (request case Abortable(:final abortTrigger as Future<void>)) {
    abortTrigger.recover(() {}); /* expect: discarded-futures */
  }
}
