import 'package:flutter/material.dart' as f;

class WrongBoundaries extends f.State<f.StatefulWidget> {
  void work() {}

  @override
  void initState() {
    work();
    super.initState(); /* expect: proper-super-calls, msg: "Place `super.initState()` as the first statement in `initState()`." */
  }

  @override
  void didUpdateWidget(f.StatefulWidget oldWidget) {
    work();
    super.didUpdateWidget(oldWidget); /* expect: proper-super-calls */
  }

  @override
  void activate() {
    work();
    super.activate(); /* expect: proper-super-calls */
  }

  @override
  void didChangeDependencies() {
    work();
    super.didChangeDependencies();
  }

  @override
  void deactivate() {
    super.deactivate(); /* expect: proper-super-calls */
    work();
  }

  @override
  void dispose() {
    super.dispose(); /* expect: proper-super-calls, msg: "Place `super.dispose()` as the last statement in `dispose()`." */
    work();
  }
}

class ConditionalCalls extends f.State<f.StatefulWidget> {
  bool ready = true;

  @override
  void initState() {
    if (ready) {
      super.initState(); /* expect: proper-super-calls */
    }
  }

  @override
  void dispose() {
    if (ready) {
      super.dispose(); /* expect: proper-super-calls */
    }
  }

  @override
  void activate() {
    {
      super.activate(); /* expect: proper-super-calls */
    }
  }

  @override
  void deactivate() {
    try {
      ready = false;
    } finally {
      super.deactivate(); /* expect: proper-super-calls */
    }
    ready = true;
  }
}

mixin WrongCleanup<T extends f.StatefulWidget> on f.State<T> {
  void work() {}

  @override
  void initState() {
    work();
    super.initState(); /* expect: proper-super-calls */
  }

  @override
  void didUpdateWidget(T oldWidget) {
    work();
    super.didUpdateWidget(oldWidget); /* expect: proper-super-calls */
  }

  @override
  void activate() {
    work();
    super.activate(); /* expect: proper-super-calls */
  }

  @override
  void deactivate() {
    super.deactivate(); /* expect: proper-super-calls */
    work();
  }

  @override
  void dispose() {
    super.dispose(); /* expect: proper-super-calls */
    work();
  }
}
