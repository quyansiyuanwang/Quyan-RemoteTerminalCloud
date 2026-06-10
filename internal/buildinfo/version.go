package buildinfo

import (
	"os"
	"path/filepath"
	"strings"
)

var Version = "0.0.0"
var ServerBaseURL = "http://localhost:10001"

func LoadVersionFromFile() string {
	root, err := repoRoot()
	if err != nil {
		return Version
	}

	content, err := os.ReadFile(filepath.Join(root, "VERSION"))
	if err != nil {
		return Version
	}

	value := strings.TrimSpace(string(content))
	if value == "" {
		return Version
	}

	Version = value
	return Version
}

func repoRoot() (string, error) {
	dir, err := os.Getwd()
	if err != nil {
		return "", err
	}

	for {
		if _, statErr := os.Stat(filepath.Join(dir, "go.mod")); statErr == nil {
			return dir, nil
		}

		parent := filepath.Dir(dir)
		if parent == dir {
			return "", os.ErrNotExist
		}
		dir = parent
	}
}
