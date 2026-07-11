import 'package:firebase_auth/firebase_auth.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:google_sign_in/google_sign_in.dart';
import '../../core/config/app_config.dart';

final firebaseAuthProvider = Provider<FirebaseAuth>(
  (ref) => FirebaseAuth.instance,
);
final authControllerProvider = AsyncNotifierProvider<AuthController, User?>(
  AuthController.new,
);

class AuthController extends AsyncNotifier<User?> {
  @override
  Future<User?> build() async => ref.watch(firebaseAuthProvider).currentUser;

  Future<void> signInWithGoogle() async {
    state = const AsyncLoading();
    state = await AsyncValue.guard(() async {
      final signIn = GoogleSignIn.instance;
      await signIn.initialize(
        serverClientId: AppConfig.googleServerClientId.isEmpty
            ? null
            : AppConfig.googleServerClientId,
      );
      final account = await signIn.authenticate();
      final idToken = account.authentication.idToken;
      if (idToken == null) throw StateError('UNCONFIGURED: Google ID token');
      final credential = GoogleAuthProvider.credential(idToken: idToken);
      return (await ref
              .read(firebaseAuthProvider)
              .signInWithCredential(credential))
          .user;
    });
  }

  Future<void> signOut() async {
    await ref.read(firebaseAuthProvider).signOut();
    await GoogleSignIn.instance.signOut();
    state = const AsyncData(null);
  }
}
