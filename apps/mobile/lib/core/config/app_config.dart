class AppConfig {
  static const firebaseEnabled = bool.fromEnvironment('FIREBASE_ENABLED');
  static const apiBaseUrl = String.fromEnvironment('HOUSEMOUSE_API_URL');
  static const googleServerClientId = String.fromEnvironment(
    'GOOGLE_SERVER_CLIENT_ID',
  );

  static String? validate() {
    if (!firebaseEnabled) {
      return 'FIREBASE_ENABLED';
    }
    final uri = Uri.tryParse(apiBaseUrl);
    if (uri == null || !uri.hasScheme || !uri.hasAuthority) {
      return 'HOUSEMOUSE_API_URL';
    }
    return null;
  }
}
