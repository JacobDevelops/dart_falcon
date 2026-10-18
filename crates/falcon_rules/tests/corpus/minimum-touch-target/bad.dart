import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

void targets() {
  SizedBox(width: 47, child: IconButton()); /* expect: minimum-touch-target */
  SizedBox(height: 24.0, child: FloatingActionButton()); /* expect: minimum-touch-target */
  f.Container(width: 0, child: GestureDetector(onTap: () {})); /* expect: minimum-touch-target */
  new SizedBox(width: 32, height: 32, child: InkWell(onTap: () {})); /* expect: minimum-touch-target */
  const SizedBox(height: (47.5), child: TextButton()); /* expect: minimum-touch-target */
  Container(width: -1, child: ElevatedButton()); /* expect: minimum-touch-target */
  SizedBox(width: 43, child: CupertinoButton(onPressed: () {})); /* expect: minimum-touch-target */
}
