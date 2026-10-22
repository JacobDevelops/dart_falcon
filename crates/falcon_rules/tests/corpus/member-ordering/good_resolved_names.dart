class State<T> {}

class LocalState extends State<int> {
  void build() {}
  void dispose() {}
  void initState() {}
}

class LocalMixinState = State<int> with Lifecycle;

mixin Lifecycle {
  void build() {}
  void dispose() {}
  void initState() {}
}
