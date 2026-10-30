import 'package:flutter/material.dart' as f;

class CorrectBoundaries extends f.State<f.StatefulWidget> {
  void work() {}

  @override
  void initState() {
    super.initState();
    work();
  }

  @override
  void didUpdateWidget(f.StatefulWidget oldWidget) {
    super.didUpdateWidget(oldWidget);
    work();
  }

  @override
  void activate() {
    (super.activate());
    work();
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    work();
  }

  @override
  void deactivate() {
    work();
    super.deactivate();
  }

  @override
  void dispose() {
    work();
    super.dispose();
  }
}

// Missing calls are the responsibility of the existing presence rules.
class MissingCalls extends f.State<f.StatefulWidget> {
  @override
  void initState() {}
  @override
  void didUpdateWidget(f.StatefulWidget oldWidget) {}
  @override
  void activate() {}
  @override
  void didChangeDependencies() {}
  @override
  void deactivate() {}
  @override
  void dispose() {}
}

class DeferredCalls extends f.State<f.StatefulWidget> {
  void defer(void Function() callback) {}

  @override
  void initState() {
    defer(() {
      super.initState();
    });
    void later() {
      super.initState();
    }
    defer(later);
  }

  @override
  void dispose() {
    void later() => super.dispose();
    defer(later);
    defer(() => super.dispose());
  }

  // A call in a non-lifecycle method is outside the rule's scope.
  void custom() {
    defer(() {});
    super.activate();
  }
}

class FinallyCleanup extends f.State<f.StatefulWidget> {
  bool ready = true;

  @override
  void deactivate() {
    try {
      ready = false;
    } finally {
      super.deactivate();
    }
  }

  @override
  void dispose() {
    try {
      ready = false;
    } finally {
      print('cleanup');
      super.dispose();
    }
  }
}

mixin Cleanup<T extends f.StatefulWidget> on f.State<T> {
  void work() {}

  @override
  void initState() {
    super.initState();
    work();
  }

  @override
  void didUpdateWidget(T oldWidget) {
    super.didUpdateWidget(oldWidget);
    work();
  }

  @override
  void activate() {
    super.activate();
    work();
  }

  @override
  void deactivate() {
    work();
    super.deactivate();
  }

  @override
  void dispose() {
    work();
    super.dispose();
  }

  @override
  void didChangeDependencies() {
    work();
    super.didChangeDependencies();
    work();
  }
}
