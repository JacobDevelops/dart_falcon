import 'package:flutter/material.dart' as f show Text;
import 'package:flutter/foundation.dart' show kDebugMode;
import 'dart:async' show Future;

f.Text label() => f.Text(kDebugMode ? 'debug' : 'release');
Future<void> later() async {}
