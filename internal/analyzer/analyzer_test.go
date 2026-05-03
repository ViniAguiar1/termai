package analyzer

import "testing"

func TestAnalyzeKnownErrors(t *testing.T) {
	tests := []struct {
		name        string
		errorOutput string
		wantTitle   string
		wantActions int
		wantRisks   []RiskLevel
	}{
		{
			name:        "detects disk full",
			errorOutput: "write /tmp/file: no space left on device",
			wantTitle:   "Disco cheio detectado",
			wantActions: 3,
			wantRisks:   []RiskLevel{RiskLow, RiskHigh, RiskMedium},
		},
		{
			name:        "detects enospc case insensitive",
			errorOutput: "ENOSPC: System limit for number of file watchers reached",
			wantTitle:   "Disco cheio detectado",
			wantActions: 3,
			wantRisks:   []RiskLevel{RiskLow, RiskHigh, RiskMedium},
		},
		{
			name:        "detects command not found",
			errorOutput: "sh: unknown-tool: command not found",
			wantTitle:   "Comando não encontrado",
			wantActions: 2,
			wantRisks:   []RiskLevel{RiskLow, RiskLow},
		},
		{
			name:        "detects port in use",
			errorOutput: "listen tcp :3000: bind: address already in use",
			wantTitle:   "Porta já está em uso",
			wantActions: 3,
			wantRisks:   []RiskLevel{RiskLow, RiskHigh, RiskLow},
		},
		{
			name:        "detects eaddrinuse",
			errorOutput: "Error: listen EADDRINUSE: address already in use :::3000",
			wantTitle:   "Porta já está em uso",
			wantActions: 3,
			wantRisks:   []RiskLevel{RiskLow, RiskHigh, RiskLow},
		},
		{
			name:        "detects module not found",
			errorOutput: "Error: Cannot find module 'next'",
			wantTitle:   "Módulo não encontrado",
			wantActions: 3,
			wantRisks:   []RiskLevel{RiskMedium, RiskMedium, RiskLow},
		},
		{
			name:        "detects permission denied",
			errorOutput: "bash: ./script.sh: permission denied",
			wantTitle:   "Permissão negada",
			wantActions: 2,
			wantRisks:   []RiskLevel{RiskMedium, RiskHigh},
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

			for i, wantRisk := range tt.wantRisks {
				if got.Actions[i].Risk != wantRisk {
					t.Fatalf("Actions[%d].Risk = %q, want %q", i, got.Actions[i].Risk, wantRisk)
				}
			}
		})
	}
}

func TestAnalyzeRiskyActionsRequireConfirmation(t *testing.T) {
	got := Analyze("Error: Cannot find module 'next'")
	if got == nil {
		t.Fatal("Analyze returned nil")
	}

	for _, action := range got.Actions {
		if action.Risk == RiskMedium || action.Risk == RiskHigh {
			if !action.RequiresConfirmation {
				t.Fatalf("action %q should require confirmation", action.Label)
			}
		}
	}
}

func TestAnalyzeUnknownError(t *testing.T) {
	got := Analyze("some random warning without a known pattern")
	if got != nil {
		t.Fatalf("Analyze returned %#v, want nil", got)
	}
}
