class AppConfig {
  static const firebaseEnabled = bool.fromEnvironment('FIREBASE_ENABLED');
  static const apiBaseUrl = String.fromEnvironment('HOUSEMOUSE_API_URL');
  static const googleServerClientId = String.fromEnvironment(
    'GOOGLE_SERVER_CLIENT_ID',
  );
  static const sentryDsn = String.fromEnvironment('SENTRY_DSN');

  static String? validate() {
    if (!firebaseEnabled) {
      return 'FIREBASE_ENABLED';
    }
    final uri = Uri.tryParse(apiBaseUrl);
    if (uri == null || !uri.hasScheme || !uri.hasAuthority) {
      return 'HOUSEMOUSE_API_URL';
    }
    if (sentryDsn.isNotEmpty) {
      if (!isValidSentryDsn(sentryDsn)) {
        return 'SENTRY_DSN';
      }
    }
    return null;
  }
}

bool isValidSentryDsn(String value) {
  final uri = Uri.tryParse(value);
  return uri != null && uri.scheme == 'https' && uri.hasAuthority;
}
