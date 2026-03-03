import 'package:flutter/material.dart';

// Abstract base class for localizations
abstract class AppLocalizations {
  AppLocalizations(this.localeName);

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
  _AppLocalizationsDelegate();

  static const List<Locale> supportedLocales = [
    Locale('en'),
    Locale('nb', 'NO'),
  ];

  // Common strings
  String get ok;
  String get cancel;
  String get close;
  String get settings;
  String get theme;
  String get language;
  String get light;
  String get dark;
  String get system;
  String get english;
  String get norwegian;
  String get appTitle;
  String get map;
  String get profile;
  String get guestUser;
  String get notSignedIn;
  String get turboUser;
  String get areYouSureYouWantToLogout;
  String get login;
  String get logout;
  String get or;

  // Auth Screen
  String get signIn;
  String toContinueTo(String appName);
  String get email;
  String get password;
  String get pleaseEnterEmail;
  String get pleaseEnterValidEmail;
  String get pleaseEnterPassword;
  String passwordTooShort(int minLength);
  String get forgotPassword;
  String get dontHaveAnAccount;
  String get createAccount;
  String get alreadyHaveAnAccount;
  String get termsAndPrivacy;
  String get closeTooltip;
  String get signInWithGoogle;
  String get errorCouldNotLaunchUrl;
  String signInFailed(String error);

  // Profile Screen
  String get editProfile;
  String get changePassword;
  String get notifications;
  String get helpAndSupport;
  String get about;
  String get user;
  String get delete;

  // Marker Sheets
  String get newMarker;
  String get editMarker;
  String get saveMarker;
  String get saveChanges;
  String get deleteMarker;
  String get name;
  String get pleaseEnterName;
  String get descriptionOptional;
  String get icon;
  String get selectAnIcon;
  String get searchIcons;
  String get noIconsFound;
  String get confirmDeleteTitle;
  String get confirmDeleteMessage;
  String get errorLocationNotSpecified;
  String errorUpdatingLocation(String error);
  String errorDeletingLocation(String error);
  String get nameLabel;
  String get descriptionLabel;
  String get iconLabel;

  // Icon names
  String get iconFjell;
  String get iconPark;
  String get iconStrand;
  String get iconSkog;
  String get iconVandring;
  String get iconKajakk;
  String get iconSykkel;
  String get iconHytte;
  String get iconParkering;
  String get iconCampingSpot;
  String get iconBadeplass;
  String get iconDykking;
  String get iconUtkikkspunkt;
  String get iconRestaurant;
  String get iconKafe;
  String get iconOvernatting;
  String get iconFiskeplass;
  String get iconSki;
  String get iconDefault;

  // Search
  String get searchHint;
  String get searchHintMobile;
  String get noResultsFound;
  String get menu;

  // Layer selection
  String get mapLayers;
  String get globalMaps;
  String get norwegianMaps;
  String get overlays;
  String get layerNameNorgeskart;
  String get layerDescriptionNorgeskart;
  String get layerNameOsm;
  String get layerDescriptionOsm;
  String get layerNameGoogleSatellite;
  String get layerDescriptionGoogleSatellite;
  String get layerNameAvalanche;
  String get layerDescriptionAvalanche;

  // Measuring Tool
  String get totalDistance;
  String get undoLastPoint;
  String get resetMeasurement;
  String get done;
  String get save;
  String get needMorePoints;
  String get addDescription;
  String get showPoints;

  // Main Map
  String get createNewMarkerHere;
  String get measureDistanceFromHere;

  // Location Button
  String get locationServicesUnavailable;
  String get locationServicesUnsupportedOnPlatform;
  String get locationError;
  String get openSettings;
  String get locationServicesDisabled;
  String get locationPermissionsDenied;
  String get locationPermissionsDeniedForever;

  // Other UI
  String get devMode;
  String get googleAuth;
  String get processingGoogleLogin;
  String get loginSuccessfulRedirecting;
  String loginFailed(String error);
  String get noAuthCodeFound;
  String errorProcessingLogin(String error);
  String get continueToApp;
  String get returnToLogin;
  String get loginSuccessful;
  String welcomeBack(String email);
  String get redirectingToApp;

  String get toggleIntermediatePoints;

  String get smoothLine;

  String get drawMode;

  String get drawing;
  String get drawSensitivity;

  // My Location settings
  String get myLocation;
  String get locationIcon;
  String get chooseIcon;
  String get chooseFromGallery;
  String get takePhoto;
  String get resetToDefault;
  String get locationMarkerSize;
  String get showHeadingArrow;
  String get headingArrowDescription;

  // Follow mode
  String get followMyLocation;
  String get stopFollowing;
  String get following;

  // Compass mode
  String get compassOrientation;
  String get compassOrientationDescription;
  String get stopCompass;

  // Navigation
  String get navigateToHere;
  String get stopNavigation;
  String get navigationAhead;
  String navigationTurnLeft(int degrees);
  String navigationTurnRight(int degrees);
  String get youHaveArrived;

  // Saved Paths
  String get savedPaths;
  String get savePath;
  String get editPath;
  String get deletePath;
  String get pathName;
  String get pathSaved;
  String get confirmDeletePathTitle;
  String get discardPath;
  String get dataLayers;
  String get showMarkers;
  String get showPaths;
  String get pathUpdated;
  String get pathDeleted;
  String errorSavingPath(String error);
  String errorDeletingPath(String error);

  // Path Customization
  String get appearance;
  String get pathColor;
  String get pathIcon;
  String get pathSmoothing;
  String get pathLineStyle;
  String get lineStyleSolid;
  String get lineStyleDotted;
  String get lineStyleDashed;
  String get lineStyleDashDot;
  String get defaultColor;
  String get removeIcon;

  // Export
  String get exportPath;
  String get selectFormat;
  String get gpxDescription;
  String get geoJsonDescription;
  String get share;
  String get saveToFile;
  String get pathExported;
  String errorExportingPath(String error);

  // Info Sheets
  String get coordinates;
  String get createdDate;
  String get edit;
  String get export;
  String get exportMarker;
  String get textDescription;
  String get shareAsText;
  String get shareAsFile;
  String get markerExported;
  String get markerDeleted;
  String get markerUpdated;
  String errorExportingMarker(String error);
}

// English translations
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn() : super('en');

  @override String get ok => 'OK';
  @override String get cancel => 'Cancel';
  @override String get close => 'Close';
  @override String get settings => 'Settings';
  @override String get theme => 'Theme';
  @override String get language => 'Language';
  @override String get light => 'Light';
  @override String get dark => 'Dark';
  @override String get system => 'System';
  @override String get english => 'English';
  @override String get norwegian => 'Norwegian';
  @override String get appTitle => 'Turbo';
  @override String get map => 'Map';
  @override String get profile => 'Profile';
  @override String get guestUser => 'Guest User';
  @override String get notSignedIn => 'Not signed in';
  @override String get turboUser => 'Turbo User';
  @override String get areYouSureYouWantToLogout => 'Are you sure you want to logout?';
  @override String get login => 'Login';
  @override String get logout => 'Logout';
  @override String get or => 'or';

  @override String get signIn => 'Sign in';
  @override String toContinueTo(String appName) => 'to continue to $appName';
  @override String get email => 'Email';
  @override String get password => 'Password';
  @override String get pleaseEnterEmail => 'Please enter your email';
  @override String get pleaseEnterValidEmail => 'Please enter a valid email';
  @override String get pleaseEnterPassword => 'Please enter your password';
  @override String passwordTooShort(int minLength) => 'Password must be at least $minLength characters';
  @override String get forgotPassword => 'Forgot Password?';
  @override String get dontHaveAnAccount => "Don't have an account?";
  @override String get createAccount => 'Create account';
  @override String get alreadyHaveAnAccount => 'Already have an account?';
  @override String get termsAndPrivacy => 'By creating an account, you agree to our Terms of Service and Privacy Policy.';
  @override String get closeTooltip => 'Close';
  @override String get signInWithGoogle => 'Sign in with Google';
  @override String get errorCouldNotLaunchUrl => 'Could not launch Google auth URL';
  @override String signInFailed(String error) => 'Sign in failed: $error';

  @override String get editProfile => 'Edit Profile';
  @override String get changePassword => 'Change Password';
  @override String get notifications => 'Notifications';
  @override String get helpAndSupport => 'Help & Support';
  @override String get about => 'About';
  @override String get user => 'User';
  @override String get delete => 'Delete';

  @override String get newMarker => 'New Marker';
  @override String get editMarker => 'Edit Marker';
  @override String get saveMarker => 'Save Marker';
  @override String get saveChanges => 'Save Changes';
  @override String get deleteMarker => 'Delete Marker';
  @override String get name => 'Name';
  @override String get pleaseEnterName => 'Please enter a name';
  @override String get descriptionOptional => 'Description (optional)';
  @override String get icon => 'Icon';
  @override String get selectAnIcon => 'Select an Icon';
  @override String get searchIcons => 'Search icons...';
  @override String get noIconsFound => 'No icons found.';
  @override String get confirmDeleteTitle => 'Delete Marker?';
  @override String get confirmDeleteMessage => 'This action is permanent and cannot be undone.';
  @override String get errorLocationNotSpecified => 'Location not specified for new marker.';
  @override String errorUpdatingLocation(String error) => 'Error updating location: $error';
  @override String errorDeletingLocation(String error) => 'Error deleting location: $error';
  @override String get nameLabel => 'Name';
  @override String get descriptionLabel => 'Description';
  @override String get iconLabel => 'Icon';

  @override String get iconFjell => 'Mountain';
  @override String get iconPark => 'Park';
  @override String get iconStrand => 'Beach';
  @override String get iconSkog => 'Forest';
  @override String get iconVandring => 'Hiking';
  @override String get iconKajakk => 'Kayaking';
  @override String get iconSykkel => 'Biking';
  @override String get iconHytte => 'Cabin';
  @override String get iconParkering => 'Parking';
  @override String get iconCampingSpot => 'Camping Spot';
  @override String get iconBadeplass => 'Swimming Spot';
  @override String get iconDykking => 'Diving';
  @override String get iconUtkikkspunkt => 'Viewpoint';
  @override String get iconRestaurant => 'Restaurant';
  @override String get iconKafe => 'Café';
  @override String get iconOvernatting => 'Accommodation';
  @override String get iconFiskeplass => 'Fishing Spot';
  @override String get iconSki => 'Skiing';
  @override String get iconDefault => 'Default';

  @override String get searchHint => 'Search places, coordinates...';
  @override String get searchHintMobile => 'Search places...';
  @override String get noResultsFound => 'No results found.';
  @override String get menu => 'Menu';

  @override String get mapLayers => 'Map Layers';
  @override String get globalMaps => 'Global Maps';
  @override String get norwegianMaps => 'Norwegian Maps';
  @override String get overlays => 'Overlays';
  @override String get layerNameNorgeskart => 'Norgeskart';
  @override String get layerDescriptionNorgeskart => 'Norwegian Topographic Map';
  @override String get layerNameOsm => 'Open Street Map';
  @override String get layerDescriptionOsm => 'OpenStreetMap Standard';
  @override String get layerNameGoogleSatellite => 'Google Satellite';
  @override String get layerDescriptionGoogleSatellite => 'Satellite imagery from Google';
  @override String get layerNameAvalanche => 'Avalanche Danger';
  @override String get layerDescriptionAvalanche => 'Overlay of slope steepness and run-out zones';

  @override String get totalDistance => 'Total Distance';
  @override String get undoLastPoint => 'Undo Last Point';
  @override String get resetMeasurement => 'Reset Measurement';
  @override String get done => 'Done';
  @override String get save => 'Save';
  @override String get needMorePoints => 'Add at least 2 points';
  @override String get addDescription => 'Add description';
  @override String get showPoints => 'Show points';

  @override String get createNewMarkerHere => 'Create New Marker Here';
  @override String get measureDistanceFromHere => 'Measure Distance From Here';

  @override String get locationServicesUnavailable => 'Location Services Unavailable';
  @override String get locationServicesUnsupportedOnPlatform => 'Location services are not supported on this platform. We apologize for the inconvenience.';
  @override String get locationError => 'Location Error';
  @override String get openSettings => 'Open Settings';
  @override String get locationServicesDisabled => 'Location services are disabled.';
  @override String get locationPermissionsDenied => 'Location permissions are denied.';
  @override String get locationPermissionsDeniedForever => 'Location permissions are permanently denied. Please enable them in the app settings.';


  @override String get devMode => 'Development mode';
  @override String get googleAuth => 'Google Authentication';
  @override String get processingGoogleLogin => 'Processing Google login...';
  @override String get loginSuccessfulRedirecting => 'Login successful! Redirecting...';
  @override String loginFailed(String error) => 'Login failed: $error';
  @override String get noAuthCodeFound => 'No authorization code found in the URL';
  @override String errorProcessingLogin(String error) => 'Error processing login: $error';
  @override String get continueToApp => 'Continue to App';
  @override String get returnToLogin => 'Return to Login';
  @override String get loginSuccessful => 'Login Successful!';
  @override String welcomeBack(String email) => 'Welcome back, $email';
  @override String get redirectingToApp => 'Redirecting to app...';

  @override
  String get toggleIntermediatePoints => 'Toggle intermediate points';
  @override String get smoothLine => 'Smooth Line';
  @override String get drawMode => 'Draw mode';
  @override String get drawing => 'Drawing';
  @override String get drawSensitivity => 'Draw Sensitivity';

  @override String get myLocation => 'My Location';
  @override String get locationIcon => 'Icon';
  @override String get chooseIcon => 'Choose Icon';
  @override String get chooseFromGallery => 'Choose from Gallery';
  @override String get takePhoto => 'Take Photo';
  @override String get resetToDefault => 'Reset to Default';
  @override String get locationMarkerSize => 'Marker Size';
  @override String get showHeadingArrow => 'Show Heading Arrow';
  @override String get headingArrowDescription => 'Shows the direction your phone is pointing';
  @override String get followMyLocation => 'Follow My Location';
  @override String get stopFollowing => 'Stop Following';
  @override String get following => 'Following';
  @override String get compassOrientation => 'Compass Orientation';
  @override String get compassOrientationDescription => 'Rotate the map to match your phone\'s compass heading';
  @override String get stopCompass => 'Stop Compass';

  @override String get navigateToHere => 'Navigate Here';
  @override String get stopNavigation => 'Stop Navigation';
  @override String get navigationAhead => 'Ahead';
  @override String navigationTurnLeft(int degrees) => '$degrees° left';
  @override String navigationTurnRight(int degrees) => '$degrees° right';
  @override String get youHaveArrived => 'You have arrived!';

  @override String get savedPaths => 'Saved Paths';
  @override String get savePath => 'Save Path';
  @override String get editPath => 'Edit Path';
  @override String get deletePath => 'Delete Path';
  @override String get pathName => 'Path Name';
  @override String get pathSaved => 'Path saved';
  @override String get confirmDeletePathTitle => 'Delete Path?';
  @override String get discardPath => 'Discard';
  @override String get dataLayers => 'Data';
  @override String get showMarkers => 'Markers';
  @override String get showPaths => 'Paths';
  @override String get pathUpdated => 'Path updated';
  @override String get pathDeleted => 'Path deleted';
  @override String errorSavingPath(String error) => 'Error saving path: $error';
  @override String errorDeletingPath(String error) => 'Error deleting path: $error';

  @override String get appearance => 'Appearance';
  @override String get pathColor => 'Color';
  @override String get pathIcon => 'Icon';
  @override String get pathSmoothing => 'Smooth line';
  @override String get pathLineStyle => 'Line style';
  @override String get lineStyleSolid => 'Solid';
  @override String get lineStyleDotted => 'Dotted';
  @override String get lineStyleDashed => 'Dashed';
  @override String get lineStyleDashDot => 'Dash-dot';
  @override String get defaultColor => 'Default';
  @override String get removeIcon => 'Remove icon';

  @override String get exportPath => 'Export Path';
  @override String get selectFormat => 'Select Format';
  @override String get gpxDescription => 'GPS Exchange Format - works with most GPS apps';
  @override String get geoJsonDescription => 'GeoJSON - works with mapping and GIS tools';
  @override String get share => 'Share';
  @override String get saveToFile => 'Save to File';
  @override String get pathExported => 'Path exported successfully';
  @override String errorExportingPath(String error) => 'Error exporting path: $error';

  @override String get coordinates => 'Coordinates';
  @override String get createdDate => 'Created';
  @override String get edit => 'Edit';
  @override String get export => 'Export';
  @override String get exportMarker => 'Export Marker';
  @override String get textDescription => 'Plain text - easy to paste anywhere';
  @override String get shareAsText => 'Share as text';
  @override String get shareAsFile => 'Share as file';
  @override String get markerExported => 'Marker saved';
  @override String get markerDeleted => 'Marker deleted';
  @override String get markerUpdated => 'Marker updated';
  @override String errorExportingMarker(String error) => 'Error exporting marker: $error';
}

// Norwegian translations
class AppLocalizationsNo extends AppLocalizations {
  AppLocalizationsNo() : super('nb');

  @override String get ok => 'OK';
  @override String get cancel => 'Avbryt';
  @override String get close => 'Lukk';
  @override String get settings => 'Innstillinger';
  @override String get theme => 'Tema';
  @override String get language => 'Språk';
  @override String get light => 'Lys';
  @override String get dark => 'Mørk';
  @override String get system => 'System';
  @override String get english => 'Engelsk';
  @override String get norwegian => 'Norsk';
  @override String get appTitle => 'Turbo';
  @override String get map => 'Kart';
  @override String get profile => 'Profil';
  @override String get guestUser => 'Gjestebruker';
  @override String get notSignedIn => 'Ikke logget inn';
  @override String get turboUser => 'Turbo-bruker';
  @override String get areYouSureYouWantToLogout => 'Er du sikker på at du vil logge ut?';
  @override String get login => 'Logg inn';
  @override String get logout => 'Logg ut';
  @override String get or => 'eller';

  @override String get signIn => 'Logg inn';
  @override String toContinueTo(String appName) => 'for å fortsette til $appName';
  @override String get email => 'E-post';
  @override String get password => 'Passord';
  @override String get pleaseEnterEmail => 'Vennligst skriv inn din e-post';
  @override String get pleaseEnterValidEmail => 'Vennligst skriv inn en gyldig e-post';
  @override String get pleaseEnterPassword => 'Vennligst skriv inn ditt passord';
  @override String passwordTooShort(int minLength) => 'Passordet må være minst $minLength tegn';
  @override String get forgotPassword => 'Glemt passord?';
  @override String get dontHaveAnAccount => 'Har du ikke en konto?';
  @override String get createAccount => 'Opprett konto';
  @override String get alreadyHaveAnAccount => 'Har du allerede en konto?';
  @override String get termsAndPrivacy => 'Ved å opprette en konto godtar du våre Betingelser for bruk og Personvernerklæring.';
  @override String get closeTooltip => 'Lukk';
  @override String get signInWithGoogle => 'Logg inn med Google';
  @override String get errorCouldNotLaunchUrl => 'Kunne ikke åpne Google-innloggingssiden';
  @override String signInFailed(String error) => 'Innlogging feilet: $error';

  @override String get editProfile => 'Rediger profil';
  @override String get changePassword => 'Endre passord';
  @override String get notifications => 'Varslinger';
  @override String get helpAndSupport => 'Hjelp og støtte';
  @override String get about => 'Om';
  @override String get user => 'Bruker';
  @override String get delete => 'Slett';

  @override String get newMarker => 'Nytt punkt';
  @override String get editMarker => 'Rediger punkt';
  @override String get saveMarker => 'Lagre punkt';
  @override String get saveChanges => 'Lagre endringer';
  @override String get deleteMarker => 'Slett punkt';
  @override String get name => 'Navn';
  @override String get pleaseEnterName => 'Vennligst skriv inn et navn';
  @override String get descriptionOptional => 'Beskrivelse (valgfritt)';
  @override String get icon => 'Ikon';
  @override String get selectAnIcon => 'Velg et ikon';
  @override String get searchIcons => 'Søk etter ikoner...';
  @override String get noIconsFound => 'Ingen ikoner funnet.';
  @override String get confirmDeleteTitle => 'Slette punktet?';
  @override String get confirmDeleteMessage => 'Denne handlingen er permanent og kan ikke angres.';
  @override String get errorLocationNotSpecified => 'Posisjon er ikke spesifisert for nytt punkt.';
  @override String errorUpdatingLocation(String error) => 'Feil ved oppdatering av punkt: $error';
  @override String errorDeletingLocation(String error) => 'Feil ved sletting av punkt: $error';
  @override String get nameLabel => 'Navn';
  @override String get descriptionLabel => 'Beskrivelse';
  @override String get iconLabel => 'Ikon';

  @override String get iconFjell => 'Fjell';
  @override String get iconPark => 'Park';
  @override String get iconStrand => 'Strand';
  @override String get iconSkog => 'Skog';
  @override String get iconVandring => 'Vandring';
  @override String get iconKajakk => 'Kajakk';
  @override String get iconSykkel => 'Sykkel';
  @override String get iconHytte => 'Hytte';
  @override String get iconParkering => 'Parkering';
  @override String get iconCampingSpot => 'Campingplass';
  @override String get iconBadeplass => 'Badeplass';
  @override String get iconDykking => 'Dykking';
  @override String get iconUtkikkspunkt => 'Utkikkspunkt';
  @override String get iconRestaurant => 'Restaurant';
  @override String get iconKafe => 'Kafé';
  @override String get iconOvernatting => 'Overnatting';
  @override String get iconFiskeplass => 'Fiskeplass';
  @override String get iconSki => 'Ski';
  @override String get iconDefault => 'Standard';

  @override String get searchHint => 'Søk etter steder, koordinater...';
  @override String get searchHintMobile => 'Søk etter steder...';
  @override String get noResultsFound => 'Ingen resultater funnet.';
  @override String get menu => 'Meny';

  @override String get mapLayers => 'Kartlag';
  @override String get globalMaps => 'Globale kart';
  @override String get norwegianMaps => 'Norske kart';
  @override String get overlays => 'Overlegg';
  @override String get layerNameNorgeskart => 'Norgeskart';
  @override String get layerDescriptionNorgeskart => 'Norsk topografisk kart';
  @override String get layerNameOsm => 'Open Street Map';
  @override String get layerDescriptionOsm => 'OpenStreetMap Standard';
  @override String get layerNameGoogleSatellite => 'Google Satellitt';
  @override String get layerDescriptionGoogleSatellite => 'Satellittbilder fra Google';
  @override String get layerNameAvalanche => 'Snøskredfare';
  @override String get layerDescriptionAvalanche => 'Oversikt over bratthet og utløpssoner';

  @override String get totalDistance => 'Total distanse';
  @override String get undoLastPoint => 'Angre siste punkt';
  @override String get resetMeasurement => 'Nullstill måling';
  @override String get done => 'Ferdig';
  @override String get save => 'Lagre';
  @override String get needMorePoints => 'Legg til minst 2 punkter';
  @override String get addDescription => 'Legg til beskrivelse';
  @override String get showPoints => 'Vis punkter';

  @override String get createNewMarkerHere => 'Opprett nytt punkt her';
  @override String get measureDistanceFromHere => 'Mål avstand herfra';

  @override String get locationServicesUnavailable => 'Posisjonstjenester utilgjengelig';
  @override String get locationServicesUnsupportedOnPlatform => 'Posisjonstjenester støttes ikke på denne plattformen. Vi beklager ulempen.';
  @override String get locationError => 'Posisjonsfeil';
  @override String get openSettings => 'Åpne innstillinger';
  @override String get locationServicesDisabled => 'Posisjonstjenester er deaktivert.';
  @override String get locationPermissionsDenied => 'Posisjonstilgang er avslått.';
  @override String get locationPermissionsDeniedForever => 'Posisjonstilgang er permanent avslått. Vennligst aktiver tilgang i app-innstillingene.';

  @override String get devMode => 'Utviklingsmodus';
  @override String get googleAuth => 'Google-autentisering';
  @override String get processingGoogleLogin => 'Behandler Google-innlogging...';
  @override String get loginSuccessfulRedirecting => 'Innlogging vellykket! Omdirigerer...';
  @override String loginFailed(String error) => 'Innlogging feilet: $error';
  @override String get noAuthCodeFound => 'Ingen autorisasjonskode funnet i URL-en';
  @override String errorProcessingLogin(String error) => 'Feil ved behandling av innlogging: $error';
  @override String get continueToApp => 'Fortsett til appen';
  @override String get returnToLogin => 'Tilbake til innlogging';
  @override String get loginSuccessful => 'Innlogging vellykket!';
  @override String welcomeBack(String email) => 'Velkommen tilbake, $email';
  @override String get redirectingToApp => 'Omdirigerer til appen...';
  @override String get toggleIntermediatePoints => 'Vis mellompunkter';
  @override String get smoothLine => 'Kurvet linje';
  @override String get drawMode => 'Tegnemodus';
  @override String get drawing => 'Tegning';
  @override String get drawSensitivity => 'Tegnefølsomhet';

  @override String get myLocation => 'Min posisjon';
  @override String get locationIcon => 'Ikon';
  @override String get chooseIcon => 'Velg ikon';
  @override String get chooseFromGallery => 'Velg fra galleri';
  @override String get takePhoto => 'Ta bilde';
  @override String get resetToDefault => 'Tilbakestill til standard';
  @override String get locationMarkerSize => 'Markørstørrelse';
  @override String get showHeadingArrow => 'Vis retningspil';
  @override String get headingArrowDescription => 'Viser retningen telefonen peker';
  @override String get followMyLocation => 'Følg min posisjon';
  @override String get stopFollowing => 'Slutt å følge';
  @override String get following => 'Følger';
  @override String get compassOrientation => 'Kompassorientering';
  @override String get compassOrientationDescription => 'Roter kartet etter telefonens kompassretning';
  @override String get stopCompass => 'Stopp kompass';

  @override String get navigateToHere => 'Naviger hit';
  @override String get stopNavigation => 'Stopp navigering';
  @override String get navigationAhead => 'Rett frem';
  @override String navigationTurnLeft(int degrees) => '$degrees° venstre';
  @override String navigationTurnRight(int degrees) => '$degrees° høyre';
  @override String get youHaveArrived => 'Du har ankommet!';

  @override String get savedPaths => 'Lagrede ruter';
  @override String get savePath => 'Lagre rute';
  @override String get editPath => 'Rediger rute';
  @override String get deletePath => 'Slett rute';
  @override String get pathName => 'Rutenavn';
  @override String get pathSaved => 'Rute lagret';
  @override String get confirmDeletePathTitle => 'Slette ruten?';
  @override String get discardPath => 'Forkast';
  @override String get dataLayers => 'Data';
  @override String get showMarkers => 'Markorer';
  @override String get showPaths => 'Ruter';
  @override String get pathUpdated => 'Rute oppdatert';
  @override String get pathDeleted => 'Rute slettet';
  @override String errorSavingPath(String error) => 'Feil ved lagring av rute: $error';
  @override String errorDeletingPath(String error) => 'Feil ved sletting av rute: $error';

  @override String get appearance => 'Utseende';
  @override String get pathColor => 'Farge';
  @override String get pathIcon => 'Ikon';
  @override String get pathSmoothing => 'Kurvet linje';
  @override String get pathLineStyle => 'Linjestil';
  @override String get lineStyleSolid => 'Heltrukken';
  @override String get lineStyleDotted => 'Prikket';
  @override String get lineStyleDashed => 'Stiplet';
  @override String get lineStyleDashDot => 'Strek-prikk';
  @override String get defaultColor => 'Standard';
  @override String get removeIcon => 'Fjern ikon';

  @override String get exportPath => 'Eksporter rute';
  @override String get selectFormat => 'Velg format';
  @override String get gpxDescription => 'GPS Exchange Format - fungerer med de fleste GPS-apper';
  @override String get geoJsonDescription => 'GeoJSON - fungerer med kart- og GIS-verktøy';
  @override String get share => 'Del';
  @override String get saveToFile => 'Lagre til fil';
  @override String get pathExported => 'Rute eksportert';
  @override String errorExportingPath(String error) => 'Feil ved eksportering av rute: $error';

  @override String get coordinates => 'Koordinater';
  @override String get createdDate => 'Opprettet';
  @override String get edit => 'Rediger';
  @override String get export => 'Eksporter';
  @override String get exportMarker => 'Eksporter punkt';
  @override String get textDescription => 'Ren tekst - enkelt å lime inn hvor som helst';
  @override String get shareAsText => 'Del som tekst';
  @override String get shareAsFile => 'Del som fil';
  @override String get markerExported => 'Punkt lagret';
  @override String get markerDeleted => 'Punkt slettet';
  @override String get markerUpdated => 'Punkt oppdatert';
  @override String errorExportingMarker(String error) => 'Feil ved eksportering av punkt: $error';
}

// The delegate
class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  bool isSupported(Locale locale) =>
      ['en', 'nb'].contains(locale.languageCode);

  @override
  Future<AppLocalizations> load(Locale locale) async {
    switch (locale.languageCode) {
      case 'nb':
        return AppLocalizationsNo();
      case 'en':
      default:
        return AppLocalizationsEn();
    }
  }

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

// Helper extension for easier access
extension AppLocalizationsX on BuildContext {
  AppLocalizations get l10n => AppLocalizations.of(this);
}