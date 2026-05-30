# Push notifications — setup guide

The app and API ship with push-notification **scaffolding**: device-token
registration, an `IPushSender` abstraction with an FCM HTTP v1 implementation,
and a Flutter `PushNotificationService` that registers tokens and renders
foreground messages. None of it requires credentials to build or run — it stays
inert until you complete the steps below.

This is intentional: provisioning Firebase Cloud Messaging (FCM) and Apple Push
Notification service (APNs) requires accounts and signing assets that can't live
in the repo.

## What already works (no setup)

- `POST /api/auth/Devices` / `POST /api/auth/Devices/unregister` — authenticated
  device-token registration (`device_tokens` table).
- `IPushSender` / `FcmPushSender` — fan-out to a user's tokens + FCM HTTP v1
  payload construction. No-op (logs only) until `Notifications:Fcm` is set.
- Flutter `PushNotificationService` — defensively initializes Firebase, requests
  permission, registers/refreshes the token while signed-in with push enabled,
  unregisters on logout, and shows foreground notifications. Defensive init means
  a build without Firebase config simply logs and does nothing.
- Notification preferences UI (Settings → Notifications), persisted locally.

## 1. Create the Firebase project

1. Create a Firebase project at <https://console.firebase.google.com>.
2. Add an **Android app** (package `com.example.turbo` — confirm the real
   `applicationId` in `apps/flutter/android/app/build.gradle`).
3. Add an **iOS app** (matching the bundle id in Xcode).

## 2. Flutter / client config

Recommended: use the FlutterFire CLI from `apps/flutter`:

```bash
dart pub global activate flutterfire_cli
flutterfire configure
```

This generates `lib/firebase_options.dart` and wires the native config. If you
prefer manual setup:

- **Android**: place `google-services.json` in `apps/flutter/android/app/`, add
  the `com.google.gms.google-services` Gradle plugin, and add the
  `POST_NOTIFICATIONS` permission (Android 13+) to the manifest.
  `flutter_local_notifications` also requires Java 8+ core-library desugaring —
  set `isCoreLibraryDesugaringEnabled true` and add the
  `coreLibraryDesugaring` dependency per that package's README.
- **iOS**: place `GoogleService-Info.plist` in `apps/flutter/ios/Runner/`, enable
  the **Push Notifications** capability and **Background Modes → Remote
  notifications** in Xcode, and upload an APNs key to the Firebase project.

If you generate `firebase_options.dart`, pass the options to
`Firebase.initializeApp` in
`apps/flutter/lib/features/notifications/push_notification_service.dart`
(`_initFirebase`). With native config files in place, the no-argument
`Firebase.initializeApp()` also works.

## 3. API / server config

Provide FCM credentials via the `Notifications:Fcm` configuration section
(env vars shown):

```
Notifications__Fcm__ProjectId=<firebase-project-id>
Notifications__Fcm__ServiceAccountJson=<inline service account JSON>
# or
Notifications__Fcm__ServiceAccountJsonPath=/run/secrets/fcm-service-account.json
```

Generate the service account under Firebase Console → Project settings →
Service accounts → *Generate new private key*.

### Finish `FcmPushSender`

`FcmPushSender.TryGetAccessTokenAsync` currently returns `null` (the only thing
keeping live delivery off when credentials are present). Implement it with the
service account, e.g. add the `Google.Apis.Auth` package and:

```csharp
var credential = GoogleCredential
    .FromJson(serviceAccountJson)
    .CreateScoped("https://www.googleapis.com/auth/firebase.messaging");
return await credential.UnderlyingCredential.GetAccessTokenForRequestAsync();
```

Once it returns a token, `IPushSender.IsConfigured` is already true and
`SendAsync` will deliver to FCM.

## 4. Trigger notifications

Inject `IPushSender` where an event should notify a user and call `SendAsync`.
Natural first triggers live in the Sharing module (friend requests, new shares).
Stale tokens (FCM 404/410) are pruned automatically.

## Testing

- `device_tokens` registration is covered by the API behaviour tests.
- End-to-end delivery requires a real device/emulator with the Firebase config
  in place; it can't run in CI without credentials.
