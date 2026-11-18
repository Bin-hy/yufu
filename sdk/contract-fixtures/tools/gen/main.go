package main

import (
	"bytes"
	"crypto"
	"crypto/rand"
	"crypto/rsa"
	"crypto/sha256"
	"crypto/x509"
	"encoding/base64"
	"encoding/json"
	"encoding/pem"
	"flag"
	"fmt"
	"io/fs"
	"math/big"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type manifest struct {
	Version string `json:"version"`
	Issuer  string `json:"issuer"`
	Clock   int64  `json:"clock"`
	KID     string `json:"signing_kid"`
	Files   struct {
		Discovery    string            `json:"discovery"`
		JWKS         string            `json:"jwks"`
		Endpoints    string            `json:"endpoints"`
		Expectations string            `json:"expectations"`
		Keys         map[string]string `json:"keys"`
	} `json:"files"`
}

type claimSpec struct {
	ID     string         `json:"id"`
	Key    string         `json:"key"`
	Header map[string]any `json:"header"`
	Claims map[string]any `json:"claims"`
}

type jwk struct {
	Kty string `json:"kty"`
	Use string `json:"use"`
	Alg string `json:"alg"`
	Kid string `json:"kid"`
	N   string `json:"n"`
	E   string `json:"e"`
}

type jwks struct {
	Keys []jwk `json:"keys"`
}

func main() {
	root := flag.String("root", "../..", "contract fixture root directory")
	out := flag.String("out", "", "output directory (defaults to root)")
	flag.Parse()
	if *out == "" {
		*out = *root
	}
	if err := generate(*root, *out); err != nil {
		fmt.Fprintln(os.Stderr, "generate:", err)
		os.Exit(1)
	}
}

// generate reads the hand-written sources under root and writes the derived
// artifacts under out. It is deterministic: identical inputs produce identical
// bytes.
func generate(root, out string) error {
	var m manifest
	if err := readJSON(filepath.Join(root, "manifest.json"), &m); err != nil {
		return err
	}
	if m.KID == "" {
		return fmt.Errorf("manifest signing_kid is required")
	}
	for _, path := range []string{
		m.Files.Discovery,
		m.Files.Endpoints,
		m.Files.Expectations,
	} {
		if _, err := os.Stat(filepath.Join(root, path)); err != nil {
			return fmt.Errorf("manifest references %s: %w", path, err)
		}
	}
	keys := make(map[string]*rsa.PrivateKey, len(m.Files.Keys))
	for name, rel := range m.Files.Keys {
		key, err := loadPrivateKey(filepath.Join(root, rel))
		if err != nil {
			return fmt.Errorf("load %s key: %w", name, err)
		}
		keys[name] = key
	}
	if keys["signing"] == nil {
		return fmt.Errorf("manifest is missing the signing key")
	}

	tokens, err := signClaims(filepath.Join(root, "claims"), filepath.Join(out, "tokens"), keys)
	if err != nil {
		return err
	}
	if err := writeJWKS(filepath.Join(out, "jwks.json"), keys["signing"], m.KID); err != nil {
		return err
	}
	if err := writeResponses(root, out, tokens); err != nil {
		return err
	}
	return nil
}

func readJSON(path string, target any) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return err
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	return decoder.Decode(target)
}

func loadPrivateKey(path string) (*rsa.PrivateKey, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	block, _ := pem.Decode(data)
	if block == nil {
		return nil, fmt.Errorf("%s: no PEM block", path)
	}
	if key, err := x509.ParsePKCS1PrivateKey(block.Bytes); err == nil {
		return key, nil
	}
	parsed, err := x509.ParsePKCS8PrivateKey(block.Bytes)
	if err != nil {
		return nil, err
	}
	key, ok := parsed.(*rsa.PrivateKey)
	if !ok {
		return nil, fmt.Errorf("%s: not an RSA private key", path)
	}
	return key, nil
}

func signClaims(claimsDir, tokensDir string, keys map[string]*rsa.PrivateKey) (map[string]string, error) {
	if err := os.RemoveAll(tokensDir); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(tokensDir, 0o755); err != nil {
		return nil, err
	}
	var files []string
	err := filepath.WalkDir(claimsDir, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() || !strings.HasSuffix(path, ".json") {
			return nil
		}
		files = append(files, path)
		return nil
	})
	if err != nil {
		return nil, err
	}
	sort.Strings(files)

	tokens := make(map[string]string, len(files))
	for _, path := range files {
		var spec claimSpec
		if err := readJSON(path, &spec); err != nil {
			return nil, fmt.Errorf("%s: %w", path, err)
		}
		if spec.ID == "" {
			return nil, fmt.Errorf("%s: id is required", path)
		}
		if _, exists := tokens[spec.ID]; exists {
			return nil, fmt.Errorf("%s: duplicate id %q", path, spec.ID)
		}
		key, ok := keys[spec.Key]
		if !ok {
			return nil, fmt.Errorf("%s: unknown key %q", path, spec.Key)
		}
		if alg, _ := spec.Header["alg"].(string); alg != "RS256" {
			return nil, fmt.Errorf("%s: alg must be RS256", path)
		}
		token, err := signJWT(key, spec.Header, spec.Claims)
		if err != nil {
			return nil, fmt.Errorf("%s: %w", path, err)
		}
		tokens[spec.ID] = token

		rel, err := filepath.Rel(claimsDir, path)
		if err != nil {
			return nil, err
		}
		target := filepath.Join(tokensDir, strings.TrimSuffix(rel, ".json")+".jwt")
		if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
			return nil, err
		}
		if err := os.WriteFile(target, []byte(token+"\n"), 0o644); err != nil {
			return nil, err
		}
	}
	return tokens, nil
}

func signJWT(key *rsa.PrivateKey, header, claims map[string]any) (string, error) {
	headerJSON, err := json.Marshal(header)
	if err != nil {
		return "", err
	}
	claimsJSON, err := json.Marshal(claims)
	if err != nil {
		return "", err
	}
	signingInput := base64.RawURLEncoding.EncodeToString(headerJSON) + "." +
		base64.RawURLEncoding.EncodeToString(claimsJSON)
	digest := sha256.Sum256([]byte(signingInput))
	signature, err := rsa.SignPKCS1v15(rand.Reader, key, crypto.SHA256, digest[:])
	if err != nil {
		return "", err
	}
	return signingInput + "." + base64.RawURLEncoding.EncodeToString(signature), nil
}

func writeJWKS(path string, key *rsa.PrivateKey, kid string) error {
	public := key.PublicKey
	document := jwks{Keys: []jwk{{
		Kty: "RSA",
		Use: "sig",
		Alg: "RS256",
		Kid: kid,
		N:   base64.RawURLEncoding.EncodeToString(public.N.Bytes()),
		E:   base64.RawURLEncoding.EncodeToString(big.NewInt(int64(public.E)).Bytes()),
	}}}
	return writeJSON(path, document)
}

func writeResponses(root, out string, tokens map[string]string) error {
	sourcePath := filepath.Join(root, "responses.source.json")
	data, err := os.ReadFile(sourcePath)
	if err != nil {
		return err
	}
	decoder := json.NewDecoder(bytes.NewReader(data))
	decoder.UseNumber()
	var source map[string]any
	if err := decoder.Decode(&source); err != nil {
		return err
	}
	responsesDir := filepath.Join(out, "responses")
	if err := os.RemoveAll(responsesDir); err != nil {
		return err
	}
	if err := os.MkdirAll(responsesDir, 0o755); err != nil {
		return err
	}
	for name, value := range source {
		replaced, err := replaceTokenRefs(value, tokens)
		if err != nil {
			return fmt.Errorf("response %q: %w", name, err)
		}
		if err := writeJSON(filepath.Join(responsesDir, name+".json"), replaced); err != nil {
			return err
		}
	}
	return nil
}

// replaceTokenRefs substitutes {"token": "<id>"} objects with the signed JWT.
func replaceTokenRefs(value any, tokens map[string]string) (any, error) {
	switch typed := value.(type) {
	case map[string]any:
		if len(typed) == 1 {
			if id, ok := typed["token"].(string); ok {
				token, found := tokens[id]
				if !found {
					return nil, fmt.Errorf("unknown token id %q", id)
				}
				return token, nil
			}
		}
		for key, item := range typed {
			replaced, err := replaceTokenRefs(item, tokens)
			if err != nil {
				return nil, err
			}
			typed[key] = replaced
		}
		return typed, nil
	case []any:
		for i, item := range typed {
			replaced, err := replaceTokenRefs(item, tokens)
			if err != nil {
				return nil, err
			}
			typed[i] = replaced
		}
		return typed, nil
	default:
		return value, nil
	}
}

func writeJSON(path string, value any) error {
	data, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return err
	}
	data = append(data, '\n')
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	return os.WriteFile(path, data, 0o644)
}