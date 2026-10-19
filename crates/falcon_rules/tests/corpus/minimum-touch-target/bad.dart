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

void callbacks(dynamic callback) {
  SizedBox(width: 1, child: IconButton(onPressed: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: IconButton.filled(onPressed: () {})); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: FloatingActionButton.small(onPressed: (callback))); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: TextButton(onPressed: null, onLongPress: () {})); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: ElevatedButton(onPressed: (null), onLongPress: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: OutlinedButton(onPressed: () {}, onLongPress: null)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: RawMaterialButton(onPressed: callback)); /* expect: minimum-touch-target */
  SizedBox(width: 43, child: CupertinoButton(onPressed: (callback))); /* expect: minimum-touch-target */
  SizedBox(width: 43, child: CupertinoButton(onPressed: null, onLongPress: () {})); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: TextButton(onLongPress: null)); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: GestureDetector(onTap: (callback))); /* expect: minimum-touch-target */
  SizedBox(width: 1, child: InkWell(onTap: null, onLongPress: () {})); /* expect: minimum-touch-target */
}
