package cmd

import (
	"fmt"
	"os"
	"os/exec"
	"strconv"
	"strings"
)

type promptInfo struct {
	Git *gitStatus
}

type gitStatus struct {
	Branch string
	Dirty  bool
	Ahead  int
	Behind int
}

func buildPrompt() string {
	info := collectPromptInfo()

	segments := []string{
		promptColor("⚡"),
		infoColor("termAI"),
		warnColor("v" + versionString()),
	}

	if info.Git != nil {
		segments = append(segments, formatGitSegment(*info.Git))
	}

	return strings.Join(segments, " ") + " ❯ "
}

func collectPromptInfo() promptInfo {
	return promptInfo{
		Git: currentGitStatus(),
	}
}

func currentGitStatus() *gitStatus {
	if _, err := os.Stat(".git"); err != nil {
		if !isInsideGitWorkTree() {
			return nil
		}
	}

	out, err := exec.Command("git", "status", "--porcelain", "--branch").Output()
	if err != nil {
		return nil
	}

	status := parseGitStatusOutput(string(out))
	return &status
}

func isInsideGitWorkTree() bool {
	cmd := exec.Command("git", "rev-parse", "--is-inside-work-tree")
	out, err := cmd.Output()
	if err != nil {
		return false
	}

	return strings.TrimSpace(string(out)) == "true"
}

func parseGitStatusOutput(output string) gitStatus {
	lines := strings.Split(strings.TrimSpace(output), "\n")
	status := gitStatus{}

	if len(lines) == 0 || lines[0] == "" {
		return status
	}

	status.Branch, status.Ahead, status.Behind = parseGitBranchLine(lines[0])
	status.Dirty = len(lines) > 1

	return status
}

func parseGitBranchLine(line string) (branch string, ahead int, behind int) {
	line = strings.TrimPrefix(line, "## ")

	parts := strings.SplitN(line, " ", 2)
	ref := parts[0]

	branch = ref
	if head, _, found := strings.Cut(ref, "..."); found {
		branch = head
	}

	if strings.Contains(line, "[") {
		meta := line[strings.Index(line, "[")+1 : strings.LastIndex(line, "]")]
		items := strings.Split(meta, ",")
		for _, item := range items {
			item = strings.TrimSpace(item)
			if value, ok := strings.CutPrefix(item, "ahead "); ok {
				ahead, _ = strconv.Atoi(value)
			}
			if value, ok := strings.CutPrefix(item, "behind "); ok {
				behind, _ = strconv.Atoi(value)
			}
		}
	}

	return branch, ahead, behind
}

func formatGitSegment(status gitStatus) string {
	if status.Branch == "" {
		return ""
	}

	parts := []string{promptColor("on"), warnColor(status.Branch)}

	flags := make([]string, 0, 3)
	if status.Dirty {
		flags = append(flags, errorColor("!"))
	}
	if status.Ahead > 0 {
		flags = append(flags, infoColor("↑"+strconv.Itoa(status.Ahead)))
	}
	if status.Behind > 0 {
		flags = append(flags, warnColor("↓"+strconv.Itoa(status.Behind)))
	}

	if len(flags) > 0 {
		parts = append(parts, fmt.Sprintf("[%s]", strings.Join(flags, " ")))
	}

	return strings.Join(parts, " ")
}
