package executor

import "testing"

func TestRunCapturesStdout(t *testing.T) {
	got := Run("printf 'hello'")

	if got.Output != "hello" {
		t.Fatalf("Output = %q, want %q", got.Output, "hello")
	}

	if got.Error != "" {
		t.Fatalf("Error = %q, want empty", got.Error)
	}

	if got.ExitCode != 0 {
		t.Fatalf("ExitCode = %d, want 0", got.ExitCode)
	}
}

func TestRunCapturesStderrAndExitCode(t *testing.T) {
	got := Run("printf 'boom' >&2; exit 7")

	if got.Output != "" {
		t.Fatalf("Output = %q, want empty", got.Output)
	}

	if got.Error != "boom" {
		t.Fatalf("Error = %q, want %q", got.Error, "boom")
	}

	if got.ExitCode != 7 {
		t.Fatalf("ExitCode = %d, want 7", got.ExitCode)
	}
}
