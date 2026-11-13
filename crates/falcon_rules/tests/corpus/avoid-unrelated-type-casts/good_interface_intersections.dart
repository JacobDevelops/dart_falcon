abstract class Left {}
abstract interface class Right {}
class Both implements Left, Right {}
Right crossCast(Left value) => value as Right;
abstract class Game {}
mixin PositionProvider {}
class Component extends Game with PositionProvider {}
PositionProvider provider(Game value) => value as PositionProvider;
interface class CollisionSystem {}
class FlameGame {}
class GameCollision extends FlameGame implements CollisionSystem {}
FlameGame game(CollisionSystem value) => value as FlameGame;
void main() {
  assert(identical(crossCast(Both()).runtimeType, Both));
  assert(provider(Component()) is Component);
  assert(game(GameCollision()) is GameCollision);
  print('All interface and mixin casts succeeded.');
}
