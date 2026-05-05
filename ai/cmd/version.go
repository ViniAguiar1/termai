package cmd

import (
	"runtime/debug"
	"strings"
)

var appVersion = "dev"

func versionString() string {
	buildVersion, revision, modified := readBuildMetadata()
	return resolveVersion(appVersion, buildVersion, revision, modified)
}

func readBuildMetadata() (buildVersion string, revision string, modified bool) {
	info, ok := debug.ReadBuildInfo()
	if !ok {
		return "", "", false
	}

	buildVersion = info.Main.Version

	for _, setting := range info.Settings {
		switch setting.Key {
		case "vcs.revision":
			revision = setting.Value
		case "vcs.modified":
			modified = setting.Value == "true"
		}
	}

	return buildVersion, revision, modified
}

func resolveVersion(injected string, buildVersion string, revision string, modified bool) string {
	injected = strings.TrimSpace(injected)
	buildVersion = strings.TrimSpace(buildVersion)
	revision = strings.TrimSpace(revision)

	if injected != "" && injected != "dev" {
		return injected
	}

	if buildVersion != "" && buildVersion != "(devel)" {
		return buildVersion
	}

	if revision != "" {
		version := "dev-" + shortRevision(revision)
		if modified {
			version += "-dirty"
		}
		return version
	}

	if injected != "" {
		return injected
	}

	return "dev"
}

func shortRevision(revision string) string {
	if len(revision) <= 7 {
		return revision
	}

	return revision[:7]
}
