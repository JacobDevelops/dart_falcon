import 'package:flutter/material.dart' as f;
class GlobalKey extends f.Key { GlobalKey(); }
class Local { final _notFlutter = GlobalKey(); }
f.Widget localKey() => f.IconButton(key: GlobalKey());
