class Builder { Builder({required Object Function(Object) builder}); }
class SliverChildBuilderDelegate { SliverChildBuilderDelegate(Object Function(Object, int) builder); }
class PopupMenuButton { PopupMenuButton({required Object Function(Object) itemBuilder}); }
class SliverList { SliverList.builder({required Object Function(Object, int) itemBuilder}); }
class SliverGrid { SliverGrid.builder({required Object Function(Object, int) itemBuilder}); }
class ReorderableListView { ReorderableListView.builder({required Object Function(Object, int) itemBuilder}); }
class AnimatedList { AnimatedList({required Object Function(Object, int, Object) itemBuilder}); }
