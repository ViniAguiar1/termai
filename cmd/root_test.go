package cmd

import (
	"bufio"
	"strings"
	"testing"

	"github.com/ViniAguiar1/termai/internal/analyzer"
)

func TestHasPlaceholder(t *testing.T) {
	tests := []struct {
		name    string
		command string
		want    bool
	}{
		{name: "angle placeholder", command: "kill -9 <PID>", want: true},
		{name: "port placeholder", command: "lsof -i :PORTA", want: true},
		{name: "concrete command", command: "df -h", want: false},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := hasPlaceholder(tt.command)
			if got != tt.want {
				t.Fatalf("hasPlaceholder(%q) = %t, want %t", tt.command, got, tt.want)
			}
		})
	}
}

func TestConfirmAction(t *testing.T) {
	action := analyzer.Action{
		Label:                "Limpar cache do npm",
		Command:              "npm cache clean --force",
		Risk:                 analyzer.RiskMedium,
		RequiresConfirmation: true,
	}

	if !confirmAction(bufio.NewScanner(strings.NewReader("sim\n")), action) {
		t.Fatal("confirmAction should accept sim")
	}

	if confirmAction(bufio.NewScanner(strings.NewReader("\n")), action) {
		t.Fatal("confirmAction should reject empty answer")
	}
}
