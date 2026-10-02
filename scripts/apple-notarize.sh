#!/usr/bin/env bash
set -euo pipefail

#
# apple-notarize.sh
# Automated Apple Code Signing, Sandboxing, Packaging, and Notarization Pipeline
#

APP_PATH=""
KEY_PATH="${ASC_KEY_PATH:-}"
KEY_ID="${ASC_KEY_ID:-}"
ISSUER_ID="${ASC_ISSUER_ID:-}"
SIGNING_IDENTITY="${APPLE_SIGNING_IDENTITY:-}"
ENTITLEMENTS="templates/app.entitlements"
DRY_RUN=false
SKIP_STAPLE=false

usage() {
    cat << EOF
Usage: $(basename "$0") [OPTIONS] --app <path_to_app_or_binary>

Options:
    --app <path>            Path to .app bundle, binary, or package to notarize (required)
    --key-path <path>       Path to App Store Connect AuthKey_<KEY_ID>.p8 private key (or ASC_KEY_PATH)
    --key-id <id>           App Store Connect 10-character Key ID (or ASC_KEY_ID)
    --issuer-id <uuid>      App Store Connect Issuer UUID (or ASC_ISSUER_ID)
    --identity <name>       Signing identity (e.g., 'Developer ID Application: Company')
    --entitlements <path>   Path to .entitlements file (default: templates/app.entitlements)
    --dry-run               Run codesign verification and packaging without submitting to Apple
    --skip-staple           Skip stapling the notarization ticket to the app bundle
    -h, --help              Show this help message
EOF
    exit "${1:-1}"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --app)
            APP_PATH="$2"
            shift 2
            ;;
        --key-path)
            KEY_PATH="$2"
            shift 2
            ;;
        --key-id)
            KEY_ID="$2"
            shift 2
            ;;
        --issuer-id)
            ISSUER_ID="$2"
            shift 2
            ;;
        --identity)
            SIGNING_IDENTITY="$2"
            shift 2
            ;;
        --entitlements)
            ENTITLEMENTS="$2"
            shift 2
            ;;
        --dry-run)
            DRY_RUN=true
            shift
            ;;
        --skip-staple)
            SKIP_STAPLE=true
            shift
            ;;
        -h|--help)
            usage 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            usage
            ;;
    esac
done

if [[ -z "$APP_PATH" ]]; then
    echo "Error: --app is required." >&2
    usage
fi

if [[ ! -e "$APP_PATH" ]]; then
    echo "Error: Target does not exist at '$APP_PATH'" >&2
    exit 1
fi

echo "============================================================"
echo "Apple Notarization & Distribution Pipeline"
echo "Target: $APP_PATH"
echo "============================================================"

# Check system prerequisites
for cmd in codesign ditto xcrun; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Error: Required system tool '$cmd' is not installed or not in PATH." >&2
        exit 1
    fi
done

# Step 1: Code Signing with Hardened Runtime and Entitlements
if [[ -n "$SIGNING_IDENTITY" ]]; then
    echo "--> [1/4] Signing with Hardened Runtime and Entitlements..."
    if [[ ! -f "$ENTITLEMENTS" ]]; then
        echo "Warning: Entitlements file '$ENTITLEMENTS' not found. Signing without custom entitlements."
        codesign --force --options runtime --timestamp --sign "$SIGNING_IDENTITY" "$APP_PATH"
    else
        codesign --force --options runtime --timestamp --entitlements "$ENTITLEMENTS" --sign "$SIGNING_IDENTITY" "$APP_PATH"
    fi
    codesign --verify --deep --strict --verbose=2 "$APP_PATH"
    echo "  Signature verified successfully."
else
    echo "--> [1/4] Signing identity not specified; skipping codesigning step."
fi

# Step 2: Packaging into ZIP archive for notarization
ARCHIVE_PATH="${APP_PATH}.zip"
echo "--> [2/4] Packaging into archive: $ARCHIVE_PATH..."
rm -f "$ARCHIVE_PATH"
ditto -c -k --keepParent "$APP_PATH" "$ARCHIVE_PATH"
echo "  Archive created: $(ls -lh "$ARCHIVE_PATH" | awk '{print $5}')"

# Step 3: Notarization Submission
if [[ "$DRY_RUN" == true ]]; then
    echo "--> [3/4] Dry run mode enabled: Skipping upload to Apple Notary Service."
    echo "--> [4/4] Dry run complete: Packaging and signing verified."
    exit 0
fi

if [[ -z "$KEY_PATH" || -z "$KEY_ID" || -z "$ISSUER_ID" ]]; then
    echo "Error: Notarization requires --key-path, --key-id, and --issuer-id (or ASC_* env vars)." >&2
    echo "Use --dry-run to validate signing and packaging without submitting to Apple." >&2
    exit 1
fi

if [[ ! -f "$KEY_PATH" ]]; then
    echo "Error: API private key file not found: '$KEY_PATH'" >&2
    exit 1
fi

echo "--> [3/4] Submitting to Apple Notary Service (waiting for ticket)..."
xcrun notarytool submit "$ARCHIVE_PATH" \
    --key "$KEY_PATH" \
    --key-id "$KEY_ID" \
    --issuer "$ISSUER_ID" \
    --wait

# Step 4: Staple Ticket
if [[ "$SKIP_STAPLE" == false ]]; then
    echo "--> [4/4] Stapling notarization ticket to $APP_PATH..."
    xcrun stapler staple "$APP_PATH"
    xcrun stapler validate "$APP_PATH"
    echo "  Ticket stapled and validated."
else
    echo "--> [4/4] Skipping ticket stapling (--skip-staple)."
fi

echo "============================================================"
echo "Notarization pipeline completed successfully for $APP_PATH"
echo "============================================================"
