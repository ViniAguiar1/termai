package cmd

import (
	"fmt"
	"os"
	"strings"

	"github.com/ViniAguiar1/termai/ai/internal/analyzer"

	"github.com/fatih/color"
	"github.com/spf13/cobra"
)

var (
	promptColor = color.New(color.FgCyan).SprintFunc()
	errorColor  = color.New(color.FgRed).SprintFunc()
	infoColor   = color.New(color.FgGreen).SprintFunc()
	warnColor   = color.New(color.FgYellow).SprintFunc()
)

var rootCmd = &cobra.Command{
	Use:     "termai",
	Short:   "AI-powered terminal assistant",
	Version: versionString(),
	Run: func(cmd *cobra.Command, args []string) {
		reader, err := newLineReader()
		if err != nil {
			fmt.Println(err)
			os.Exit(1)
		}
		defer func() { _ = reader.Close() }()

		if err := newSession(reader).run(); err != nil {
			fmt.Println(err)
			os.Exit(1)
		}
	},
}

func Execute() {
	rootCmd.Version = versionString()

	if err := rootCmd.Execute(); err != nil {
		fmt.Println(err)
		os.Exit(1)
	}
}

func printActionGuidance(action analyzer.Action) {
	if action.Description != "" {
		fmt.Println(infoColor("Orientação:"), action.Description)
		return
	}

	fmt.Println("Essa ação é apenas uma orientação por enquanto.")
}

func hasPlaceholder(command string) bool {
	return strings.Contains(command, "<") && strings.Contains(command, ">") ||
		strings.Contains(command, "PORTA")
}

func confirmAction(reader lineReader, action analyzer.Action) bool {
	answer, err := reader.ReadLine(
		fmt.Sprintf("Esta ação tem risco %s. Confirmar execução? (s/N): ", riskLabel(action.Risk)),
	)
	if err != nil {
		return false
	}

	return answer == "s" || answer == "sim" || answer == "y" || answer == "yes"
}

func riskSuffix(action analyzer.Action) string {
	if action.Risk == "" || action.Risk == analyzer.RiskLow {
		return ""
	}

	return " " + warnColor("[risco: "+riskLabel(action.Risk)+"]")
}

func riskLabel(risk analyzer.RiskLevel) string {
	switch risk {
	case analyzer.RiskHigh:
		return "alto"
	case analyzer.RiskMedium:
		return "medio"
	case analyzer.RiskLow:
		return "baixo"
	default:
		return "desconhecido"
	}
}
