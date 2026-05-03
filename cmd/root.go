package cmd

import (
	"bufio"
	"fmt"
	"os"

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

			input := scanner.Text()

			if input == "exit" {
				fmt.Println("Encerrando termAI...")
				break
			}

			result := executor.Run(input)

			if result.Output != "" {
				fmt.Print(result.Output)
			}

			if result.ExitCode != 0 {
				fmt.Println(errorColor("❌ Erro:"), result.Error)
			} else if result.Error != "" {
				fmt.Println(result.Error)
			}

			suggestion := analyzer.Analyze(result.Error)

			if suggestion != nil {
				fmt.Println()
				fmt.Println(warnColor("⚠️ " + suggestion.Title))
				fmt.Println(suggestion.Description)

				if len(suggestion.Actions) > 0 {
					fmt.Println()
					fmt.Println(infoColor("💡 Sugestões:"))

					for _, action := range suggestion.Actions {
						fmt.Println("   →", action)
					}
				}
			}
		}
		fmt.Println("────────────────────────")
	},
}

func Execute() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Println(err)
		os.Exit(1)
	}
}
