class Game {
  static Game? singleton;

  Game() {
    assert(
      singleton == null,
      '$this instantiated, while another game $singleton '
      'declares itself to be a singleton',
    );
  }
}
