package cmd

import "testing"

func TestParseGitBranchLine(t *testing.T) {
	branch, ahead, behind := parseGitBranchLine("## main...origin/main [ahead 2, behind 1]")
	if branch != "main" {
		t.Fatalf("branch = %q, want %q", branch, "main")
	}
	if ahead != 2 {
		t.Fatalf("ahead = %d, want 2", ahead)
	}
	if behind != 1 {
		t.Fatalf("behind = %d, want 1", behind)
	}
}

func TestParseGitStatusOutput(t *testing.T) {
	status := parseGitStatusOutput("## feature/login...origin/feature/login [ahead 1]\n M cmd/root.go\n")
	if status.Branch != "feature/login" {
		t.Fatalf("Branch = %q, want %q", status.Branch, "feature/login")
	}
	if !status.Dirty {
		t.Fatal("Dirty = false, want true")
	}
	if status.Ahead != 1 {
		t.Fatalf("Ahead = %d, want 1", status.Ahead)
	}
}
