import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;
class Stable extends State<Widget> {
  final key = GlobalKey();
  Widget build() => Form(key: key, child: Widget());
  Widget value() => IconButton(key: ValueKey('stable'));
  Widget shadowed(GlobalKey Function() GlobalKey) => IconButton(key: GlobalKey());
  Widget shadowedPrefix(dynamic f) => IconButton(key: f.GlobalKey());
  Widget shadowedWidget(dynamic IconButton) => IconButton(key: GlobalKey());
}
class NotWidget { NotWidget({Object? key}); }
Object notWidget() => NotWidget(key: GlobalKey());
class MemberShadow extends State<Widget> {
  Key GlobalKey() => ValueKey('member');
  Widget build() => IconButton(key: GlobalKey());
}
Object localFactory() {
  final GlobalKey = () => ValueKey('local');
  return IconButton(key: GlobalKey());
}
class StableObjectKey extends GlobalObjectKey {
  const StableObjectKey(Object value) : super(value);
}
class ConstKey extends GlobalKey {
  const ConstKey() : super.constructor();
}
Widget objectKey(Object value) => IconButton(key: GlobalObjectKey(value));
Widget objectSubtype(Object value) => IconButton(key: StableObjectKey(value));
Widget explicitConst() => IconButton(key: const ConstKey());
Widget implicitConst() => const IconButton(key: ConstKey());
const storedWidget = IconButton(key: ConstKey());
class VideoView extends Widget { VideoView({Key? key}); }
class MaterialApp extends Widget { MaterialApp({Key? key}); }
class CachedWidgets extends State<Widget> {
  late final Widget player = VideoView(key: GlobalKey());
  final Widget eager = VideoView(key: GlobalKey());
  static final Widget shared = VideoView(key: GlobalKey());
}
final app = MaterialApp(key: GlobalKey());
