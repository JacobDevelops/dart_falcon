const modern = 1;
class Marker {
  const Marker();
}
@modern
class Example {}
@Marker()
void example() {}
const text = '@legacy';
