package main

import (
	"bytes"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"testing"
)

func TestGenerateIsDeterministic(t *testing.T) {
	root, err := filepath.Abs("../..")
	if err != nil {
		t.Fatal(err)
	}
	out := t.TempDir()
	if err := generate(root, out); err != nil {
		t.Fatalf("generate: %v", err)
	}
	for _, rel := range []string{"jwks.json", "tokens", "responses"} {
		compareTree(t, filepath.Join(root, rel), filepath.Join(out, rel))
	}
}

func compareTree(t *testing.T, want, got string) {
	t.Helper()
	wantFiles := listFiles(t, want)
	gotFiles := listFiles(t, got)
	if len(wantFiles) != len(gotFiles) {
		t.Fatalf("%s: file count want %d got %d\nwant: %v\ngot: %v",
			want, len(wantFiles), len(gotFiles), wantFiles, gotFiles)
	}
	for i := range wantFiles {
		if wantFiles[i] != gotFiles[i] {
			t.Fatalf("%s: file set differs at %d: want %q got %q",
				want, i, wantFiles[i], gotFiles[i])
		}
	}
	for _, rel := range wantFiles {
		wantBytes, err := os.ReadFile(filepath.Join(want, rel))
		if err != nil {
			t.Fatal(err)
		}
		gotBytes, err := os.ReadFile(filepath.Join(got, rel))
		if err != nil {
			t.Fatal(err)
		}
		if !bytes.Equal(wantBytes, gotBytes) {
			t.Fatalf("%s: %s differs; regenerate with `go run . -root ../..`",
				filepath.Join(want, rel), rel)
		}
	}
}

func listFiles(t *testing.T, root string) []string {
	t.Helper()
	var files []string
	info, err := os.Stat(root)
	if err != nil {
		t.Fatal(err)
	}
	if !info.IsDir() {
		return []string{""}
	}
	err = filepath.WalkDir(root, func(path string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() {
			return nil
		}
		rel, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		files = append(files, rel)
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	sort.Strings(files)
	return files
}