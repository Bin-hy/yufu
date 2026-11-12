// Package jose implements the small RS256/JWKS subset used by the Runtime SDK.
//
// The SDK deliberately keeps this surface private: callers get typed claims from
// the cobotusr package and never need to handle raw JOSE objects.
package jose

import (
	"bytes"
	"crypto"
	"crypto/rsa"
	"crypto/sha256"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"math/big"
	"strings"
)

const maxTokenSize = 16 * 1024

var (
	// ErrInvalidToken is returned for malformed or unverifiable tokens.
	ErrInvalidToken = errors.New("invalid token")
	// ErrUnsupportedAlgorithm is returned for non-RS256 tokens.
	ErrUnsupportedAlgorithm = errors.New("unsupported algorithm")
	// ErrKeyNotFound is returned when the JWKS has no key for a token's kid.
	ErrKeyNotFound = errors.New("key not found")
)

// Header is the protected JOSE header subset the SDK relies on.
type Header struct {
	Alg string `json:"alg"`
	Kid string `json:"kid"`
	Typ string `json:"typ"`
}

// JWK is one RSA signing key from a JWKS document.
type JWK struct {
	Kty string `json:"kty"`
	Kid string `json:"kid"`
	Use string `json:"use"`
	Alg string `json:"alg"`
	N   string `json:"n"`
	E   string `json:"e"`
}

// JWKS is an RFC 7517 key set.
type JWKS struct {
	Keys []JWK `json:"keys"`
}

// Key is a parsed RSA verification key.
type Key struct {
	Kid       string
	PublicKey *rsa.PublicKey
}

// ParseJWKS parses and validates an RSA JWKS document.
func ParseJWKS(data []byte) ([]Key, error) {
	var set JWKS
	if err := json.Unmarshal(data, &set); err != nil {
		return nil, fmt.Errorf("%w: %v", ErrInvalidToken, err)
	}
	if len(set.Keys) == 0 {
		return nil, fmt.Errorf("%w: empty JWKS", ErrInvalidToken)
	}
	keys := make([]Key, 0, len(set.Keys))
	for _, jwk := range set.Keys {
		if jwk.Kty != "RSA" {
			continue
		}
		if jwk.Use != "" && jwk.Use != "sig" {
			continue
		}
		if jwk.Alg != "" && jwk.Alg != "RS256" {
			continue
		}
		if jwk.Kid == "" {
			return nil, fmt.Errorf("%w: JWK is missing kid", ErrInvalidToken)
		}
		publicKey, err := parseRSAPublicKey(jwk)
		if err != nil {
			return nil, err
		}
		keys = append(keys, Key{Kid: jwk.Kid, PublicKey: publicKey})
	}
	if len(keys) == 0 {
		return nil, fmt.Errorf("%w: JWKS has no usable RS256 signing key", ErrInvalidToken)
	}
	return keys, nil
}

func parseRSAPublicKey(jwk JWK) (*rsa.PublicKey, error) {
	nBytes, err := base64.RawURLEncoding.DecodeString(jwk.N)
	if err != nil || len(nBytes) == 0 {
		return nil, fmt.Errorf("%w: invalid JWK modulus", ErrInvalidToken)
	}
	eBytes, err := base64.RawURLEncoding.DecodeString(jwk.E)
	if err != nil || len(eBytes) == 0 {
		return nil, fmt.Errorf("%w: invalid JWK exponent", ErrInvalidToken)
	}
	exponent := new(big.Int).SetBytes(eBytes)
	if !exponent.IsInt64() || exponent.Int64() < 3 || exponent.Int64() > int64(^uint(0)>>1) {
		return nil, fmt.Errorf("%w: invalid JWK exponent", ErrInvalidToken)
	}
	modulus := new(big.Int).SetBytes(nBytes)
	if modulus.BitLen() < 2048 {
		return nil, fmt.Errorf("%w: RSA key is smaller than 2048 bits", ErrInvalidToken)
	}
	return &rsa.PublicKey{N: modulus, E: int(exponent.Int64())}, nil
}

// Token is a parsed JWS compact token.
type Token struct {
	Header       Header
	Claims       map[string]any
	SigningInput []byte
	Signature    []byte
}

// Parse splits and decodes a compact JWS without verifying it.
func Parse(token string) (*Token, error) {
	if len(token) == 0 || len(token) > maxTokenSize {
		return nil, ErrInvalidToken
	}
	parts := strings.Split(token, ".")
	if len(parts) != 3 {
		return nil, ErrInvalidToken
	}
	headerBytes, err := base64.RawURLEncoding.DecodeString(parts[0])
	if err != nil {
		return nil, ErrInvalidToken
	}
	var header Header
	if err := json.Unmarshal(headerBytes, &header); err != nil {
		return nil, ErrInvalidToken
	}
	if header.Alg != "RS256" {
		return nil, ErrUnsupportedAlgorithm
	}
	if header.Kid == "" {
		return nil, ErrInvalidToken
	}
	claimsBytes, err := base64.RawURLEncoding.DecodeString(parts[1])
	if err != nil {
		return nil, ErrInvalidToken
	}
	decoder := json.NewDecoder(bytes.NewReader(claimsBytes))
	decoder.UseNumber()
	var claims map[string]any
	if err := decoder.Decode(&claims); err != nil {
		return nil, ErrInvalidToken
	}
	signature, err := base64.RawURLEncoding.DecodeString(parts[2])
	if err != nil || len(signature) == 0 {
		return nil, ErrInvalidToken
	}
	return &Token{
		Header:       header,
		Claims:       claims,
		SigningInput: []byte(parts[0] + "." + parts[1]),
		Signature:    signature,
	}, nil
}

// Verify checks the RS256 signature with the selected public key.
func (t *Token) Verify(key *rsa.PublicKey) error {
	if key == nil || key.N == nil || key.N.BitLen() < 2048 {
		return ErrInvalidToken
	}
	digest := sha256.Sum256(t.SigningInput)
	if err := rsa.VerifyPKCS1v15(key, crypto.SHA256, digest[:], t.Signature); err != nil {
		return ErrInvalidToken
	}
	return nil
}

// StringClaim returns a string claim.
func StringClaim(claims map[string]any, name string) (string, bool) {
	value, ok := claims[name]
	if !ok {
		return "", false
	}
	text, ok := value.(string)
	return text, ok
}

// NumberClaim returns an integer claim. JSON numbers are decoded with UseNumber.
func NumberClaim(claims map[string]any, name string) (int64, bool) {
	value, ok := claims[name]
	if !ok {
		return 0, false
	}
	switch number := value.(type) {
	case json.Number:
		parsed, err := number.Int64()
		return parsed, err == nil
	case float64:
		return int64(number), true
	case int64:
		return number, true
	case int:
		return int64(number), true
	default:
		return 0, false
	}
}

// BoolClaim returns a boolean claim.
func BoolClaim(claims map[string]any, name string) (bool, bool) {
	value, ok := claims[name]
	if !ok {
		return false, false
	}
	boolean, ok := value.(bool)
	return boolean, ok
}

// AudienceClaim returns the aud claim as a list. Both the string and array
// forms allowed by RFC 7519 are accepted.
func AudienceClaim(claims map[string]any) ([]string, error) {
	value, ok := claims["aud"]
	if !ok {
		return nil, fmt.Errorf("%w: missing aud", ErrInvalidToken)
	}
	switch audience := value.(type) {
	case string:
		if audience == "" {
			return nil, fmt.Errorf("%w: empty aud", ErrInvalidToken)
		}
		return []string{audience}, nil
	case []any:
		result := make([]string, 0, len(audience))
		for _, item := range audience {
			text, ok := item.(string)
			if !ok || text == "" {
				return nil, fmt.Errorf("%w: invalid aud", ErrInvalidToken)
			}
			result = append(result, text)
		}
		if len(result) == 0 {
			return nil, fmt.Errorf("%w: empty aud", ErrInvalidToken)
		}
		return result, nil
	default:
		return nil, fmt.Errorf("%w: invalid aud", ErrInvalidToken)
	}
}

// StringSliceClaim returns a string array claim.
func StringSliceClaim(claims map[string]any, name string) []string {
	value, ok := claims[name]
	if !ok {
		return nil
	}
	items, ok := value.([]any)
	if !ok {
		return nil
	}
	result := make([]string, 0, len(items))
	for _, item := range items {
		if text, ok := item.(string); ok {
			result = append(result, text)
		}
	}
	return result
}
