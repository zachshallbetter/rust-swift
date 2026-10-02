#!/usr/bin/env python3
"""
App Store Connect API JWT Token Generator

Generates ES256 JSON Web Tokens for authenticating with the Apple App Store Connect API
and Notarization services according to Apple's API specification.
"""

import argparse
import json
import os
import sys
import time
from pathlib import Path


def generate_asc_token(key_path: Path, key_id: str, issuer_id: str, duration_seconds: int = 1200) -> str:
    """
    Generate an App Store Connect JWT token signed with an ES256 private key (.p8).
    Maximum duration permitted by Apple is 1200 seconds (20 minutes).
    """
    if not key_path.exists():
        raise FileNotFoundError(f"Private key file not found: {key_path}")

    with open(key_path, "r", encoding="utf-8") as f:
        private_key = f.read()

    now = int(time.time())
    expiration = now + min(duration_seconds, 1200)

    headers = {
        "alg": "ES256",
        "kid": key_id,
        "typ": "JWT",
    }

    payload = {
        "iss": issuer_id,
        "iat": now,
        "exp": expiration,
        "aud": "appstoreconnect-v1",
    }

    try:
        import jwt
        token = jwt.encode(payload, private_key, algorithm="ES256", headers=headers)
        if isinstance(token, bytes):
            token = token.decode("utf-8")
        return token
    except ImportError:
        pass

    try:
        from cryptography.hazmat.primitives import hashes, serialization
        from cryptography.hazmat.primitives.asymmetric import ec
        import base64

        key = serialization.load_pem_private_key(private_key.encode("utf-8"), password=None)
        if not isinstance(key, ec.EllipticCurvePrivateKey):
            raise ValueError("Expected an EC private key (.p8) for ES256 signing")

        def b64url(data: bytes) -> str:
            return base64.urlsafe_b64encode(data).decode("ascii").rstrip("=")

        header_b64 = b64url(json.dumps(headers, separators=(",", ":")).encode("utf-8"))
        payload_b64 = b64url(json.dumps(payload, separators=(",", ":")).encode("utf-8"))
        signing_input = f"{header_b64}.{payload_b64}".encode("ascii")

        raw_signature = key.sign(signing_input, ec.ECDSA(hashes.SHA256()))

        from cryptography.hazmat.primitives.asymmetric.utils import decode_dss_signature
        r, s = decode_dss_signature(raw_signature)
        curve_bytes = (key.curve.key_size + 7) // 8
        r_bytes = r.to_bytes(curve_bytes, byteorder="big")
        s_bytes = s.to_bytes(curve_bytes, byteorder="big")
        jose_signature = r_bytes + s_bytes

        sig_b64 = b64url(jose_signature)
        return f"{header_b64}.{payload_b64}.{sig_b64}"
    except ImportError:
        raise RuntimeError("Neither 'jwt' (PyJWT) nor 'cryptography' is available. Please install PyJWT or cryptography.")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Generate JWT authentication token for Apple App Store Connect API."
    )
    parser.add_argument(
        "--key-id",
        default=os.environ.get("ASC_KEY_ID"),
        help="Apple App Store Connect 10-character Key ID (or ASC_KEY_ID env var)",
    )
    parser.add_argument(
        "--issuer-id",
        default=os.environ.get("ASC_ISSUER_ID"),
        help="Apple App Store Connect Issuer UUID (or ASC_ISSUER_ID env var)",
    )
    parser.add_argument(
        "--key-path",
        type=Path,
        default=Path(os.environ.get("ASC_KEY_PATH", "")) if os.environ.get("ASC_KEY_PATH") else None,
        help="Path to AuthKey_<KEY_ID>.p8 private key file (or ASC_KEY_PATH env var)",
    )
    parser.add_argument(
        "--duration",
        type=int,
        default=1200,
        help="Token validity duration in seconds (default: 1200, max: 1200)",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Output token in JSON envelope with expiration timestamp",
    )
    parser.add_argument(
        "--test",
        action="store_true",
        help="Run self-test generating a temporary ES256 key pair and verifying token output",
    )

    args = parser.parse_args()

    if args.test:
        from cryptography.hazmat.primitives.asymmetric import ec
        from cryptography.hazmat.primitives import serialization
        import tempfile

        test_key = ec.generate_private_key(ec.SECP256R1())
        pem_data = test_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption(),
        )
        with tempfile.NamedTemporaryFile("wb", suffix=".p8", delete=False) as tf:
            tf.write(pem_data)
            tf.flush()
            temp_path = Path(tf.name)

        try:
            token = generate_asc_token(temp_path, "TESTKEY123", "00000000-0000-0000-0000-000000000000", 300)
            parts = token.split(".")
            assert len(parts) == 3, "JWT must have exactly three dot-separated segments"
            print("Self-test passed: Successfully generated valid ES256 JWT")
            return 0
        finally:
            if temp_path.exists():
                temp_path.unlink()

    if not args.key_id:
        sys.stderr.write("Error: Missing --key-id or ASC_KEY_ID environment variable.\n")
        return 1
    if not args.issuer_id:
        sys.stderr.write("Error: Missing --issuer-id or ASC_ISSUER_ID environment variable.\n")
        return 1
    if not args.key_path or not args.key_path.exists():
        sys.stderr.write(f"Error: Missing or invalid --key-path: {args.key_path}\n")
        return 1

    try:
        token = generate_asc_token(args.key_path, args.key_id, args.issuer_id, args.duration)
        if args.json:
            now = int(time.time())
            exp = now + min(args.duration, 1200)
            print(json.dumps({"token": token, "issued_at": now, "expires_at": exp, "key_id": args.key_id}))
        else:
            print(token)
        return 0
    except Exception as e:
        sys.stderr.write(f"Error generating token: {e}\n")
        return 1


if __name__ == "__main__":
    sys.exit(main())
