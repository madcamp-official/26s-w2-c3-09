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
    : _dio = Dio(BaseOptions(baseUrl: AppConfig.apiBaseUrl));
  final FirebaseAuth _auth;
  final Dio _dio;

  Future<Options> _options({String? idempotencyKey}) async {
    final token = await _auth.currentUser?.getIdToken();
    if (token == null) throw StateError('UNAUTHENTICATED');
    final headers = <String, String>{'Authorization': 'Bearer $token'};
    if (idempotencyKey != null) {
      headers['Idempotency-Key'] = idempotencyKey;
    }
    return Options(headers: headers);
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
  }) async {
    final response = await _dio.post<Map<String, dynamic>>(
      path,
      data: body,
      options: await _options(idempotencyKey: idempotencyKey),
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

  Future<Map<String, dynamic>> delete(String path) async {
    final response = await _dio.delete<Map<String, dynamic>>(
      path,
      options: await _options(),
    );
    return response.data ?? <String, dynamic>{};
  }
}
