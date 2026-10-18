class State<T> {
  void initState() {}
  void dispose() {}
}
class Local extends State<int> {
  @override
  void initState() => super.initState();
  @override
  void dispose() => super.dispose();
}
