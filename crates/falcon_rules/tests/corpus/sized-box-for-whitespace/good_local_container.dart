class Container {
  Container({required this.width});
  final double width;
}
void main() {
  print(Container(width: 10).width);
  print(new Container(width: 20).width);
}
