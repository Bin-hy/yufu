// Package pkce implements RFC 7636 Proof Key for Code Exchange helpers.
package pkce

import (
	"crypto/rand"
	"crypto/sha256"
	"encoding/base64"
	"errors"
	"strings"
)

// VerifierLength is the number of random bytes used for a generated verifier.
// Base64url encoding turns this into 43 characters, the RFC 7636 minimum.
const VerifierLength = 32

// NewVerifier returns a cryptographically random RFC 7636 code verifier.
func NewVerifier() (string, error) {
	random := make([]byte, VerifierLength)
	if _, err := rand.Read(random); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(random), nil
}

// Challenge returns the S256 challenge for a verifier.
func Challenge(verifier string) string {
	sum := sha256.Sum256([]byte(verifier))
	return base64.RawURLEncoding.EncodeToString(sum[:])
}

// ValidateVerifier checks the RFC 7636 syntax and length bounds.
func ValidateVerifier(verifier string) error {
	if len(verifier) < 43 || len(verifier) > 128 {
		return errors.New("cobotusr: PKCE verifier must be between 43 and 128 characters")
	}
	for _, r := range verifier {
		switch {
		case r >= 'A' && r <= 'Z':
		case r >= 'a' && r <= 'z':
		case r >= '0' && r <= '9':
		case strings.ContainsRune("-._~", r):
		default:
			return errors.New("cobotusr: PKCE verifier contains an invalid character")
		}
	}
	return nil
}
