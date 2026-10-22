import 'package:flutter/widgets.dart' as flutter;

typedef StateAlias<T extends flutter.StatefulWidget> = flutter.State<T>;

abstract class PrefixedState extends flutter.State<flutter.StatefulWidget> {
  flutter.Widget build(flutter.BuildContext context) => throw UnimplementedError();
  void dispose() {} /* expect: member-ordering */
  void initState() {} /* expect: member-ordering */
}

abstract class AliasedState extends StateAlias<flutter.StatefulWidget> {
  flutter.Widget build(flutter.BuildContext context) => throw UnimplementedError();
  void initState() {} /* expect: member-ordering */
}
