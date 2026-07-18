// Bad: Color constructed with fewer than 8 hex digits.
import 'package:flutter/material.dart';

final c0 = Color(0xFFFFFF); /* expect: use-full-hex-values-for-flutter-colors */

final c1 = Color(0x00FF00); /* expect: use-full-hex-values-for-flutter-colors */

final c2 = Color(0xABC); /* expect: use-full-hex-values-for-flutter-colors */

final c3 = Color(0Xff0000); /* expect: use-full-hex-values-for-flutter-colors */

final c4 = Color(0xF); /* expect: use-full-hex-values-for-flutter-colors */

final c5 = Color(0xFF00FF); /* expect: use-full-hex-values-for-flutter-colors */
