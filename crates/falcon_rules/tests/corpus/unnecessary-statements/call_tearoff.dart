typedef Callback = void Function();

void action() {}

class Callable {
  void call() {}
}

class CallGetter {
  Callback get call {
    action();
    return action;
  }
}

extension FunctionEffects on Callback {
  int get effect {
    action();
    return 1;
  }
}

void tearoffs(void Function() callback, Callback? nullable, Callable callable,
    CallGetter getter) {
  callback.call; /* expect: unnecessary-statements */
  nullable?.call; /* expect: unnecessary-statements */
  callable.call; /* expect: unnecessary-statements */
  action.call; /* expect: unnecessary-statements */
  callback..call; /* expect: unnecessary-statements */
  callback.call();
  nullable?.call();
  callable.call();
  getter.call;
  getter.call();
  callback.effect;
}
