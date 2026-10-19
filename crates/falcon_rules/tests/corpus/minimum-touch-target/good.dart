import 'package:flutter/material.dart';
import 'package:flutter/material.dart' as f;

void targets(double size) {
  SizedBox(width: 48, height: 48, child: IconButton());
  Container(width: 60, child: GestureDetector(onTap: () {}));
  SizedBox(width: size, child: InkWell());
  SizedBox(width: 16, child: Text('Decoration'));
  SizedBox(child: FloatingActionButton());
  IconButton();
  SizedBox(width: 16);
  SizedBox(width: 44, height: 44, child: CupertinoButton(onPressed: () {}));
  SizedBox(width: 1, child: GestureDetector());
  SizedBox(width: 1, child: InkWell(onTap: null));
  SizedBox(width: 1, child: InkResponse());
  SizedBox(width: 1, child: IconButton(onPressed: null));
  f.Container(height: 1, child: f.IconButton.filled(onPressed: (null)));
  SizedBox(width: 1, child: FloatingActionButton(onPressed: null));
  SizedBox(width: 1, child: FloatingActionButton.small(onPressed: (null)));
  SizedBox(width: 1, child: TextButton(onPressed: null));
  SizedBox(width: 1, child: ElevatedButton(onPressed: (null), onLongPress: null));
  SizedBox(width: 1, child: OutlinedButton(onPressed: null, onLongPress: (null)));
  SizedBox(width: 1, child: RawMaterialButton(onPressed: null));
  SizedBox(width: 1, child: CupertinoButton(onPressed: ((null))));
}
void shadow(dynamic SizedBox) { SizedBox(width: 1, child: IconButton()); }
