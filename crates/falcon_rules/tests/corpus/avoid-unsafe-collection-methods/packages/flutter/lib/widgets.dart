class BuildContext {}
class Widget {}
typedef IndexedWidgetBuilder = Widget Function(BuildContext context, int index);
class ListView extends Widget {
  ListView.builder({required int itemCount, required IndexedWidgetBuilder itemBuilder});
}
class GridView extends Widget {
  GridView.builder({required int itemCount, required IndexedWidgetBuilder itemBuilder});
}
class Text extends Widget {
  Text(String text);
}
