package analyzer

import "testing"

func TestAnalyzeKnownErrors(t *testing.T) {
	tests := []struct {
		name        string
		errorOutput string
		wantTitle   string
		wantActions int
	}{
		{
			name:        "detects disk full",
			errorOutput: "write /tmp/file: no space left on device",
			wantTitle:   "Disco cheio detectado",
			wantActions: 3,
		},
		{
			name:        "detects enospc case insensitive",
			errorOutput: "ENOSPC: System limit for number of file watchers reached",
			wantTitle:   "Disco cheio detectado",
			wantActions: 3,
		},
		{
			name:        "detects command not found",
			errorOutput: "sh: unknown-tool: command not found",
			wantTitle:   "Comando não encontrado",
			wantActions: 2,
		},
		{
			name:        "detects port in use",
			errorOutput: "listen tcp :3000: bind: address already in use",
			wantTitle:   "Porta já está em uso",
			wantActions: 3,
		},
		{
			name:        "detects eaddrinuse",
			errorOutput: "Error: listen EADDRINUSE: address already in use :::3000",
			wantTitle:   "Porta já está em uso",
			wantActions: 3,
		},
		{
			name:        "detects module not found",
			errorOutput: "Error: Cannot find module 'next'",
			wantTitle:   "Módulo não encontrado",
			wantActions: 3,
		},
		{
			name:        "detects permission denied",
			errorOutput: "bash: ./script.sh: permission denied",
			wantTitle:   "Permissão negada",
			wantActions: 2,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := Analyze(tt.errorOutput)
			if got == nil {
				t.Fatal("Analyze returned nil")
			}

			if got.Title != tt.wantTitle {
				t.Fatalf("Title = %q, want %q", got.Title, tt.wantTitle)
			}

			if len(got.Actions) != tt.wantActions {
				t.Fatalf("len(Actions) = %d, want %d", len(got.Actions), tt.wantActions)
			}
		})
	}
}

func TestAnalyzeUnknownError(t *testing.T) {
	got := Analyze("some random warning without a known pattern")
	if got != nil {
		t.Fatalf("Analyze returned %#v, want nil", got)
	}
}
