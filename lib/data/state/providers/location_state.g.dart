// GENERATED CODE - DO NOT MODIFY BY HAND

part of 'location_state.dart';

// **************************************************************************
// RiverpodGenerator
// **************************************************************************

String _$locationStateHash() => r'839821cccb5a460dfe2d8e2a0881c870213b15db';

/// See also [LocationState].
@ProviderFor(LocationState)
final locationStateProvider =
    AutoDisposeNotifierProvider<LocationState, AsyncValue<LatLng?>>.internal(
  LocationState.new,
  name: r'locationStateProvider',
  debugGetCreateSourceHash: const bool.fromEnvironment('dart.vm.product')
      ? null
      : _$locationStateHash,
  dependencies: null,
  allTransitiveDependencies: null,
);

typedef _$LocationState = AutoDisposeNotifier<AsyncValue<LatLng?>>;
String _$locationSettingsProvHash() =>
    r'7b7b4164c4b0c82e30e5a132f169dc87d58c3bf8';

/// See also [LocationSettingsProv].
@ProviderFor(LocationSettingsProv)
final locationSettingsProvProvider = AutoDisposeNotifierProvider<
    LocationSettingsProv, LocationAccuracy>.internal(
  LocationSettingsProv.new,
  name: r'locationSettingsProvProvider',
  debugGetCreateSourceHash: const bool.fromEnvironment('dart.vm.product')
      ? null
      : _$locationSettingsProvHash,
  dependencies: null,
  allTransitiveDependencies: null,
);

typedef _$LocationSettingsProv = AutoDisposeNotifier<LocationAccuracy>;
// ignore_for_file: type=lint
// ignore_for_file: subtype_of_sealed_class, invalid_use_of_internal_member, invalid_use_of_visible_for_testing_member, inference_failure_on_uninitialized_variable, inference_failure_on_function_return_type, inference_failure_on_untyped_parameter, deprecated_member_use_from_same_package
