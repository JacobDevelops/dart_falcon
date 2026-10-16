class Image {}

extension ImageOperations on Image {
  static Future<Image> fromPixels() async => Image();

  Future<Image> transformPixels() async {
    return await fromPixels(); /* expect: unnecessary-await-in-return */
  }
}
