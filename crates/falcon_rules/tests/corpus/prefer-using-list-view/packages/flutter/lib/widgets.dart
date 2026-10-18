class Widget { const Widget(); }
class Border { const Border(); Border.all({dynamic color, double width = 1, dynamic style, double strokeAlign = -1}); const Border.fromBorderSide(dynamic side); }
class BorderSide { const BorderSide({dynamic color, double width = 1}); }
class Image extends Widget { const Image({dynamic image, String? semanticLabel, bool excludeFromSemantics = false, dynamic opacity}); Image.asset(String name, {String? semanticLabel, bool excludeFromSemantics = false}); Image.network(String name, {String? semanticLabel, bool excludeFromSemantics = false}); Image.file(dynamic file, {String? semanticLabel, bool excludeFromSemantics = false}); Image.memory(dynamic bytes, {String? semanticLabel, bool excludeFromSemantics = false}); }
class Opacity extends Widget { const Opacity({required double opacity, Widget? child}); }
class SizedBox extends Widget { const SizedBox({double? width, double? height, Widget? child, dynamic key}); const SizedBox.square({double? dimension, Widget? child}); const SizedBox.shrink(); const SizedBox.expand(); }
class Padding extends Widget { const Padding({required dynamic padding, Widget? child, dynamic key}); }
class Container extends Widget { Container({Widget? child, dynamic key, dynamic padding, dynamic margin, dynamic alignment, double? width, double? height, dynamic constraints, dynamic transform, dynamic color, dynamic decoration, dynamic foregroundDecoration}); }
class Column extends Widget { const Column({List<Widget> children = const [], dynamic key, dynamic mainAxisSize, dynamic mainAxisAlignment, dynamic crossAxisAlignment}); }
class Row extends Widget { const Row({List<Widget> children = const []}); }
class SingleChildScrollView extends Widget { const SingleChildScrollView({Widget? child, dynamic scrollDirection}); }
class ListView extends Widget { ListView({List<Widget> children = const []}); }
class GestureDetector extends Widget { GestureDetector({Widget? child, void Function()? onTap, void Function()? onLongPress, void Function()? onDoubleTap}); }
class TextButton extends Widget { TextButton({Widget? child, void Function()? onPressed}); }
class ElevatedButton extends Widget { ElevatedButton({Widget? child, void Function()? onPressed}); }
class OutlinedButton extends Widget { OutlinedButton({Widget? child, void Function()? onPressed}); }
class IconButton extends Widget { IconButton({Widget? icon, void Function()? onPressed}); }
class FloatingActionButton extends Widget { FloatingActionButton({Widget? child, void Function()? onPressed}); }
class CupertinoButton extends Widget { CupertinoButton({Widget? child, void Function()? onPressed}); }
class InkWell extends Widget { InkWell({Widget? child, void Function()? onTap}); }
class InkResponse extends Widget { InkResponse({Widget? child, void Function()? onTap}); }
class ListTile extends Widget { ListTile({void Function()? onTap}); }
