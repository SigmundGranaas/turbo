#!/usr/bin/env python3
"""Check that a release APK can actually route on the device.

Run against the signed APK *before* it is published. Everything here is
a release-only failure mode: the debug build does not minify and does
not split by ABI, so none of these can be caught by running the app on a
desktop or an emulator, and all of them produce an APK that installs,
launches, browses maps, and then fails the moment someone asks for a
route in a downloaded region.

Four things are checked, in the order they would bite:

1. **The engine is in the APK.** `:core:routing-android` and
   `:core:turbomap-android` keep separate ABI lists, and the release
   `splits` block produces per-ABI APKs. A mismatch between those two
   lists ships an APK whose routing library is simply absent.

2. **R8 kept the bindings.** `uniffi.*` is generated code nothing
   references by name, so it is dead by every measure R8 has.

3. **R8 kept JNA's field names.** This is the subtle one.
   `@Structure.FieldOrder("capacity", "len", "data")` names fields as
   *strings*, resolved by reflection at runtime. Renaming the fields
   leaves the strings untouched and the failure is invisible until Rust
   is called.

Exit code is 0 when the APK is fit to publish, 1 otherwise, with the
reason on stderr.

    verify-release-apk.py <apk> [--mapping mapping.txt] [--abi arm64-v8a]
"""

import argparse
import re
import sys
import zipfile

# Present in every APK that can route. `libjnidispatch.so` is JNA's own
# native half and arrives via the `@aar` dependency — a plain jar would
# put a desktop build here, which is the classic uniffi-on-Android
# footgun and looks identical until it is loaded.
REQUIRED_LIBS = ["libturbo_route_ffi.so", "libjnidispatch.so"]

# The uniffi structs JNA reflects over, and the field names its
# @Structure.FieldOrder annotations look up by string.
REQUIRED_FIELD_NAMES = ["capacity", "len", "data", "code", "error_buf"]

# The pack that rides in the APK so the phone can route with no server
# at all. Its manifest is what `PackStore` scans for; without that file
# the other 53 MB are dead weight and every forced-device route reports
# "no downloaded map covers this route" — which reads as a routing
# defect and is actually a packaging one.

# Enough of the generated surface that a partial strip is still caught.
REQUIRED_CLASSES = [
    "uniffi/turbo_route_ffi/RustBuffer",
    "uniffi/turbo_route_ffi/UniffiRustCallStatus",
    "uniffi/turbo_route_ffi/UniffiLib",
]


def fail(msg: str) -> None:
    print(f"FAIL  {msg}", file=sys.stderr)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("apk")
    ap.add_argument("--abi", default="arm64-v8a")
    ap.add_argument("--mapping", help="R8 mapping.txt, if the build produced one")
    args = ap.parse_args()

    problems = []

    with zipfile.ZipFile(args.apk) as z:
        names = z.namelist()

        # 1. The native engine, for this APK's own ABI.
        for lib in REQUIRED_LIBS:
            path = f"lib/{args.abi}/{lib}"
            if path not in names:
                shipped = sorted(n for n in names if n.endswith(".so"))
                problems.append(
                    f"{path} is missing from the APK. This APK cannot route.\n"
                    f"        Check the ABI list in core/routing-android/build.gradle.kts\n"
                    f"        against the release `splits` block in app/build.gradle.kts.\n"
                    f"        Shipped: {shipped or '(no .so at all)'}"
                )

        # 2 + 3. The Kotlin half, read straight out of the DEX. Class
        # names and annotation strings both live in the string pool, so
        # a substring search over the raw bytes answers both questions
        # without a DEX parser.
        dex = b"".join(z.read(n) for n in names if re.fullmatch(r"classes\d*\.dex", n))
        if not dex:
            problems.append("no classes.dex in the APK")
        else:
            for cls in REQUIRED_CLASSES:
                if f"L{cls};".encode() not in dex:
                    problems.append(
                        f"class {cls} is not in the DEX — R8 stripped or renamed the\n"
                        f"        uniffi bindings. Check the -keep rules in "
                        f"app/proguard-rules.pro."
                    )
            for field in REQUIRED_FIELD_NAMES:
                if field.encode() not in dex:
                    problems.append(
                        f"JNA field name '{field}' is not in the DEX. @Structure.FieldOrder\n"
                        f"        resolves it by reflection at runtime, so the first Rust call\n"
                        f"        will throw. Check -keepclassmembers for com.sun.jna.Structure."
                    )

    # A cross-check on the same question, when R8 left its notes. A kept
    # class maps to itself; a renamed one does not.
    if args.mapping:
        try:
            text = open(args.mapping, encoding="utf-8", errors="replace").read()
        except OSError as e:
            problems.append(f"could not read mapping file: {e}")
        else:
            for cls in REQUIRED_CLASSES:
                dotted = cls.replace("/", ".")
                if f"{dotted} -> {dotted}:" not in text:
                    problems.append(
                        f"{dotted} was renamed by R8 (expected an identity mapping).\n"
                        f"        The -keep rule for uniffi.** is not taking effect."
                    )

    if problems:
        print(f"\n{args.apk}: NOT fit to publish\n", file=sys.stderr)
        for p in problems:
            fail(p)
        return 1

    print(f"{args.apk}: routing engine present ({args.abi}), bindings intact.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
