class State<T> {
  void initState() {}
  void didUpdateWidget(T oldWidget) {}
  void activate() {}
  void didChangeDependencies() {}
  void deactivate() {}
  void dispose() {}
}

class LocalState extends State<Object> {
  @override
  void initState() {
    print('before');
    super.initState();
  }
  @override
  void didUpdateWidget(Object oldWidget) {
    print('before');
    super.didUpdateWidget(oldWidget);
  }
  @override
  void activate() {
    print('before');
    super.activate();
  }
  @override
  void didChangeDependencies() {
    print('before');
    super.didChangeDependencies();
  }
  @override
  void deactivate() {
    super.deactivate();
    print('after');
  }
  @override
  void dispose() {
    super.dispose();
    print('after');
  }
}

// A same-named constraint is not the Flutter State API.
mixin LocalCleanup<T> on State<T> {
  @override
  void initState() {
    print('before');
    super.initState();
  }

  @override
  void dispose() {
    super.dispose();
    print('after');
  }
}
