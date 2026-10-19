// Flutter-looking names are not Flutter declaration identities.
class Widget {
  const Widget();
}
class Center extends Widget {
  const Center({this.child});
  final Widget? child;
}
class Container extends Widget {
  const Container({this.child});
  final Widget? child;
}
Widget localClasses() => Center(child: Container(child: Center(child: Container(child: Center()))));
Widget explicitLocalClasses() => new Center(child: new Center(child: new Center(child: new Center())));
Widget constantLocalClasses() => const Center(child: Center(child: Center(child: Center())));
