import 'package:flutter/widgets.dart' as f;

class Panel extends f.Widget {
  const Panel({required this.child});
  const Panel.named({required this.child});
  final f.Widget child;
}
typedef PanelAlias = Panel;

f.Widget direct() => f.Center(child: f.Center(child: f.Center(child: f.Text('deep')))); /* expect: widget-tree-depth */
f.Widget explicitNew() => new f.Center(child: new f.Center(child: new f.Center(child: new f.Text('deep')))); /* expect: widget-tree-depth */
f.Widget constant() => const f.Center(child: f.Center(child: f.Center(child: f.Text('deep')))); /* expect: widget-tree-depth */
f.Widget children() => f.Column(children: [f.Center(child: f.Center(child: f.Text('deep'))), f.Text('sibling')]); /* expect: widget-tree-depth */
f.Widget conditional(bool flag) => f.Center(child: flag ? f.Center(child: f.Center(child: f.Text('deep'))) : f.Text('shallow')); /* expect: widget-tree-depth */
f.Widget custom() => Panel.named(child: PanelAlias(child: f.Center(child: f.Text('deep')))); /* expect: widget-tree-depth */
f.Widget parentheses() => f.Center(child: ((f.Center(child: (f.Center(child: f.Text('deep'))))))); /* expect: widget-tree-depth */
f.Widget collectionIf(bool flag) => f.Column(children: [if (flag) f.Center(child: f.Center(child: f.Text('deep')))]); /* expect: widget-tree-depth */
f.Widget collectionSpread() => f.Column(children: [...[f.Center(child: f.Center(child: f.Text('deep')))]]); /* expect: widget-tree-depth */
f.Widget sizedBoxIsTransparent() => f.Center(child: f.SizedBox(child: f.Center(child: f.Center(child: f.Text('deep'))))); /* expect: widget-tree-depth */

class SizedBox extends f.Widget {
  const SizedBox({required this.child});
  final f.Widget child;
}
f.Widget unrelatedSizedBoxCounts() => SizedBox(child: SizedBox(child: SizedBox(child: f.Text('deep')))); /* expect: widget-tree-depth */
