import 'dart:async';

import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:logging/logging.dart';

import 'package:turbo/features/auth/api.dart';
import 'package:turbo/features/settings/api.dart';

final _log = Logger('PushNotifications');

/// Background isolate handler. Must be a top-level / static function annotated
/// with `vm:entry-point` so it survives tree-shaking and can be invoked when
/// the app is terminated. Kept intentionally light — real handling (deep
/// links, badge counts) is wired once the product defines notification types.
@pragma('vm:entry-point')
Future<void> firebaseMessagingBackgroundHandler(RemoteMessage message) async {
  // Firebase is initialized lazily by the platform here; we only need to be a
  // valid entry-point. Logging is best-effort in the background isolate.
  _log.fine('Background push received: ${message.messageId}');
}

/// Coordinates push notifications end-to-end:
///   * defensively initializes Firebase (no-op if the app isn't configured),
///   * requests OS permission,
///   * registers/refreshes the FCM device token with the backend while the
///     user is authenticated and has push enabled, and unregisters otherwise,
///   * surfaces foreground messages via a local notification.
///
/// The whole thing is guarded so that an unconfigured build (no
/// google-services.json / APNs entitlement, or push disabled) simply logs and
/// stays inert — it never throws into app startup. See
/// docs/notifications/push-setup.md for the native configuration steps.
class PushNotificationService {
  PushNotificationService(this._ref);

  final Ref _ref;
  final FlutterLocalNotificationsPlugin _localNotifications =
      FlutterLocalNotificationsPlugin();

  bool _firebaseReady = false;
  bool _started = false;
  String? _registeredToken;
  ProviderSubscription<AuthState>? _authSub;
  StreamSubscription<String>? _tokenRefreshSub;
  StreamSubscription<RemoteMessage>? _foregroundSub;

  static const AndroidNotificationChannel _channel = AndroidNotificationChannel(
    'turbo_default',
    'General',
    description: 'General Turbo notifications',
    importance: Importance.defaultImportance,
  );

  /// Idempotent. Safe to call on every cold start; only the first call wires
  /// listeners.
  Future<void> start() async {
    if (_started) return;
    _started = true;

    if (kIsWeb) {
      // Web push needs a separate service-worker + VAPID setup; out of scope
      // for the initial scaffold.
      _log.fine('Push notifications are not enabled on web.');
      return;
    }

    _firebaseReady = await _initFirebase();
    if (!_firebaseReady) return;

    await _initLocalNotifications();

    FirebaseMessaging.onBackgroundMessage(firebaseMessagingBackgroundHandler);
    _foregroundSub = FirebaseMessaging.onMessage.listen(_showForeground);
    _tokenRefreshSub =
        FirebaseMessaging.instance.onTokenRefresh.listen((token) {
      _registeredToken = null; // force re-registration with the new token
      unawaited(_syncRegistration(token: token));
    });

    // React to login/logout. Safe to call here: the provider is mounted and
    // we're not inside a lifecycle callback.
    _authSub = _ref.listen<AuthState>(authStateProvider, (_, _) {
      unawaited(_syncRegistration());
    });

    await _syncRegistration();
  }

  Future<bool> _initFirebase() async {
    try {
      await Firebase.initializeApp();
      return true;
    } catch (e) {
      // Most commonly: no google-services.json / GoogleService-Info.plist.
      _log.info('Firebase not configured; push notifications disabled. ($e)');
      return false;
    }
  }

  Future<void> _initLocalNotifications() async {
    try {
      const initSettings = InitializationSettings(
        android: AndroidInitializationSettings('@mipmap/ic_launcher'),
        iOS: DarwinInitializationSettings(),
      );
      await _localNotifications.initialize(settings: initSettings);
      await _localNotifications
          .resolvePlatformSpecificImplementation<
              AndroidFlutterLocalNotificationsPlugin>()
          ?.createNotificationChannel(_channel);
    } catch (e) {
      _log.warning('Local notifications init failed: $e');
    }
  }

  /// Brings the backend registration in line with the current auth + preference
  /// state: registers the token when signed in and push is enabled, otherwise
  /// unregisters it.
  Future<void> _syncRegistration({String? token}) async {
    if (!_firebaseReady) return;

    final auth = _ref.read(authStateProvider);
    final authed = auth.status == AuthStatus.authenticated;
    final pushEnabled = _ref.read(settingsProvider).maybeWhen(
          data: (s) => s.pushNotificationsEnabled,
          orElse: () => true,
        );

    final authService = _ref.read(authServiceProvider);

    if (!authed || !pushEnabled) {
      final previous = _registeredToken;
      if (previous != null) {
        _registeredToken = null;
        try {
          await authService.unregisterDevice(previous);
        } catch (e) {
          _log.warning('Failed to unregister device token: $e');
        }
      }
      return;
    }

    try {
      final settings = await FirebaseMessaging.instance.requestPermission();
      if (settings.authorizationStatus == AuthorizationStatus.denied) {
        _log.info('Push permission denied; not registering.');
        return;
      }

      final fcmToken = token ?? await FirebaseMessaging.instance.getToken();
      if (fcmToken == null || fcmToken == _registeredToken) return;

      await authService.registerDevice(fcmToken, _platform());
      _registeredToken = fcmToken;
      _log.fine('Registered device token for push notifications.');
    } catch (e) {
      _log.warning('Device token registration failed: $e');
    }
  }

  Future<void> _showForeground(RemoteMessage message) async {
    final notification = message.notification;
    if (notification == null) return;
    try {
      await _localNotifications.show(
        id: notification.hashCode,
        title: notification.title,
        body: notification.body,
        notificationDetails: NotificationDetails(
          android: AndroidNotificationDetails(
            _channel.id,
            _channel.name,
            channelDescription: _channel.description,
            importance: Importance.defaultImportance,
          ),
          iOS: const DarwinNotificationDetails(),
        ),
      );
    } catch (e) {
      _log.warning('Failed to display foreground notification: $e');
    }
  }

  String _platform() {
    switch (defaultTargetPlatform) {
      case TargetPlatform.android:
        return 'android';
      case TargetPlatform.iOS:
        return 'ios';
      default:
        return 'unknown';
    }
  }

  void dispose() {
    _authSub?.close();
    unawaited(_tokenRefreshSub?.cancel());
    unawaited(_foregroundSub?.cancel());
  }
}

/// App-wide push coordinator. Kicked off during background init in `main`.
final pushNotificationServiceProvider =
    Provider<PushNotificationService>((ref) {
  final service = PushNotificationService(ref);
  ref.onDispose(service.dispose);
  return service;
});
