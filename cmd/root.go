package cmd

import (
	"bufio"
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/ViniAguiar1/termai/internal/analyzer"
	"github.com/ViniAguiar1/termai/internal/executor"

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
	Use:   "termai",
	Short: "AI-powered terminal assistant",
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Println("termAI iniciado 🚀")
		fmt.Println("Digite um comando (ou 'exit' para sair)")

		scanner := bufio.NewScanner(os.Stdin)

		for {
			fmt.Print(promptColor("⚡ termAI ❯ "))

			if !scanner.Scan() {
				break
			}

			input := strings.TrimSpace(scanner.Text())

			if input == "" {
				continue
			}

			if input == "exit" {
				fmt.Println("Encerrando termAI...")
				break
			}

			result := executor.Run(input)

			// Output
			if result.Output != "" {
				fmt.Print(result.Output)
			}

			// Erro baseado em exit code
			if result.ExitCode != 0 {
				fmt.Println(errorColor("❌ Erro:"), result.Error)
			} else if result.Error != "" {
				fmt.Println(result.Error)
			}

			// Análise de erro
			suggestion := analyzer.AnalyzeCommand(input, result.Error)

			if suggestion != nil {
				fmt.Println()
				fmt.Println(warnColor("⚠️ " + suggestion.Title))
				fmt.Println(suggestion.Description)

				if len(suggestion.Actions) > 0 {
					fmt.Println()
					fmt.Println(infoColor("💡 Sugestões:"))

					for i, action := range suggestion.Actions {
						fmt.Printf("   [%d] %s%s\n", i+1, action.Label, riskSuffix(action))
					}

					fmt.Println()
					fmt.Print("Escolha uma ação (Enter para ignorar): ")

					if !scanner.Scan() {
						break
					}

					choice := strings.TrimSpace(scanner.Text())

					if choice != "" {
						index, err := strconv.Atoi(choice)
						if err == nil && index > 0 && index <= len(suggestion.Actions) {
							selected := suggestion.Actions[index-1]

							if selected.Command != "" {
								runAction(scanner, selected)
							} else {
								printActionGuidance(selected)
							}
						} else {
							fmt.Println("Opção inválida.")
						}
					}
				}
			}

			// Separador visual (NO LUGAR CERTO)
			fmt.Println("────────────────────────")
		}
	},
}

func Execute() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Println(err)
		os.Exit(1)
	}
}

func runAction(scanner *bufio.Scanner, action analyzer.Action) {
	if hasPlaceholder(action.Command) {
		fmt.Println(warnColor("Ação precisa de edição manual:"), action.Command)
		return
	}

	if action.RequiresConfirmation && !confirmAction(scanner, action) {
		fmt.Println("Ação cancelada.")
		return
	}

	fmt.Println(infoColor("⚙️ Executando:"), action.Command)

	execResult := executor.Run(action.Command)

	if execResult.Output != "" {
		fmt.Print(execResult.Output)
	}

	if execResult.ExitCode != 0 {
		fmt.Println(errorColor("❌ Erro:"), execResult.Error)
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

func confirmAction(scanner *bufio.Scanner, action analyzer.Action) bool {
	fmt.Printf("Esta ação tem risco %s. Confirmar execução? (s/N): ", riskLabel(action.Risk))

	if !scanner.Scan() {
		return false
	}

	answer := strings.ToLower(strings.TrimSpace(scanner.Text()))

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
