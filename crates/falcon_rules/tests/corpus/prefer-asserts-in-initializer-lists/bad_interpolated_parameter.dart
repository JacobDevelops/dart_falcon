class Game {
  Game(int thisValue) {
    assert(thisValue > 0, '$thisValue must be positive'); /* expect: prefer-asserts-in-initializer-lists */
  }
}
