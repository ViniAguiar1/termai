package cmd

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strconv"
	"strings"
	"time"
)

const (
	githubLatestReleaseAPI = "https://api.github.com/repos/ViniAguiar1/termai/releases/latest"
	releasesPageURL        = "https://github.com/ViniAguiar1/termai/releases/latest"
)

// UpdateCheckRequest asks whether a newer release than CurrentVersion exists.
type UpdateCheckRequest struct {
	Type           string `json:"type"`
	CurrentVersion string `json:"current_version"`
}

// UpdateResponse reports the result of an update check.
// Type is "update_available" or "no_update".
type UpdateResponse struct {
	Type    string `json:"type"`
	Version string `json:"version,omitempty"`
	URL     string `json:"url,omitempty"`
}

type githubRelease struct {
	TagName string `json:"tag_name"`
	HTMLURL string `json:"html_url"`
}

func handleUpdateCheck(encoder *json.Encoder, req UpdateCheckRequest) {
	latest, url, err := fetchLatestRelease()
	if err != nil || latest == "" {
		// Fail quiet: no network / no release yet just means "no update".
		_ = encoder.Encode(UpdateResponse{Type: "no_update"})
		return
	}
	if compareVersions(latest, req.CurrentVersion) > 0 {
		_ = encoder.Encode(UpdateResponse{Type: "update_available", Version: latest, URL: url})
		return
	}
	_ = encoder.Encode(UpdateResponse{Type: "no_update"})
}

func fetchLatestRelease() (version string, url string, err error) {
	client := &http.Client{Timeout: 5 * time.Second}
	httpReq, err := http.NewRequest(http.MethodGet, githubLatestReleaseAPI, nil)
	if err != nil {
		return "", "", err
	}
	httpReq.Header.Set("Accept", "application/vnd.github+json")
	resp, err := client.Do(httpReq)
	if err != nil {
		return "", "", err
	}
	defer func() { _ = resp.Body.Close() }()
	if resp.StatusCode != http.StatusOK {
		return "", "", fmt.Errorf("github status %d", resp.StatusCode)
	}
	var rel githubRelease
	if err := json.NewDecoder(resp.Body).Decode(&rel); err != nil {
		return "", "", err
	}
	page := rel.HTMLURL
	if page == "" {
		page = releasesPageURL
	}
	return strings.TrimPrefix(strings.TrimSpace(rel.TagName), "v"), page, nil
}

// compareVersions returns >0 if a>b, <0 if a<b, 0 if equal, comparing dotted
// numeric cores (pre-release/build suffixes after '-' or '+' are ignored).
func compareVersions(a, b string) int {
	pa := strings.Split(versionCore(a), ".")
	pb := strings.Split(versionCore(b), ".")
	n := len(pa)
	if len(pb) > n {
		n = len(pb)
	}
	for i := 0; i < n; i++ {
		na, nb := 0, 0
		if i < len(pa) {
			na, _ = strconv.Atoi(pa[i])
		}
		if i < len(pb) {
			nb, _ = strconv.Atoi(pb[i])
		}
		if na != nb {
			if na > nb {
				return 1
			}
			return -1
		}
	}
	return 0
}

func versionCore(v string) string {
	v = strings.TrimPrefix(strings.TrimSpace(v), "v")
	if i := strings.IndexAny(v, "-+"); i >= 0 {
		return v[:i]
	}
	return v
}
