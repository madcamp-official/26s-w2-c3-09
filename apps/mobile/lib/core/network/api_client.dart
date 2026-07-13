import 'package:dio/dio.dart';
import 'package:firebase_auth/firebase_auth.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../config/app_config.dart';
import '../../features/auth/auth_controller.dart';

final apiClientProvider = Provider<ApiClient>(
  (ref) => ApiClient(ref.watch(firebaseAuthProvider)),
);

class ApiClient {
  ApiClient(this._auth)
    : _dio = Dio(
        BaseOptions(
          baseUrl: AppConfig.apiBaseUrl,
          connectTimeout: const Duration(seconds: 10),
          sendTimeout: const Duration(seconds: 15),
          receiveTimeout: const Duration(seconds: 30),
        ),
      );
  final FirebaseAuth _auth;
  final Dio _dio;

  Future<Options> _options({
    String? idempotencyKey,
    Duration? requestTimeout,
    String? expectedOwnerUid,
  }) async {
    final user = _auth.currentUser;
    if (expectedOwnerUid != null && user?.uid != expectedOwnerUid) {
      throw StateError('ACCOUNT_CHANGED');
    }
    final token = await user?.getIdToken();
    if (token == null) throw StateError('UNAUTHENTICATED');
    if (expectedOwnerUid != null &&
        _auth.currentUser?.uid != expectedOwnerUid) {
      throw StateError('ACCOUNT_CHANGED');
    }
    final headers = <String, String>{'Authorization': 'Bearer $token'};
    if (idempotencyKey != null) {
      headers['Idempotency-Key'] = idempotencyKey;
    }
    return Options(
      headers: headers,
      sendTimeout: requestTimeout,
      receiveTimeout: requestTimeout,
    );
  }

  Future<List<Map<String, dynamic>>> getList(String path) async {
    final response = await _dio.get<List<dynamic>>(
      path,
      options: await _options(),
    );
    return (response.data ?? const []).cast<Map<String, dynamic>>();
  }

  Future<Map<String, dynamic>> get(String path) async {
    final response = await _dio.get<Map<String, dynamic>>(
      path,
      options: await _options(),
    );
    return response.data ?? <String, dynamic>{};
  }

  Future<Map<String, dynamic>?> getNullable(String path) async {
    final response = await _dio.get<dynamic>(path, options: await _options());
    final data = response.data;
    return data is Map<String, dynamic> ? data : null;
  }

  Future<Map<String, dynamic>> post(
    String path,
    Map<String, dynamic> body, {
    String? idempotencyKey,
    String? expectedOwnerUid,
  }) async {
    final response = await _dio.post<Map<String, dynamic>>(
      path,
      data: body,
      options: await _options(
        idempotencyKey: idempotencyKey,
        expectedOwnerUid: expectedOwnerUid,
      ),
    );
    return response.data ?? <String, dynamic>{};
  }

  Future<void> downloadSignedUrl(
    String url,
    String destinationPath, {
    ProgressCallback? onReceiveProgress,
    CancelToken? cancelToken,
  }) async {
    await Dio().download(
      url,
      destinationPath,
      onReceiveProgress: onReceiveProgress,
      cancelToken: cancelToken,
    );
  }

  Future<Map<String, dynamic>> patch(
    String path,
    Map<String, dynamic> body,
  ) async {
    final response = await _dio.patch<Map<String, dynamic>>(
      path,
      data: body,
      options: await _options(),
    );
    return response.data ?? <String, dynamic>{};
  }

  Future<Map<String, dynamic>> delete(
    String path, {
    String? idempotencyKey,
    Duration? requestTimeout,
  }) async {
    final response = await _dio.delete<Map<String, dynamic>>(
      path,
      options: await _options(
        idempotencyKey: idempotencyKey,
        requestTimeout: requestTimeout,
      ),
    );
    return response.data ?? <String, dynamic>{};
  }
}
