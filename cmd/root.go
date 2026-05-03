package cmd

import (
	"bufio"
	"fmt"
	"os"

	"github.com/ViniAguiar1/termai/internal/analyzer"
	"github.com/ViniAguiar1/termai/internal/executor"

	"github.com/spf13/cobra"
)

var rootCmd = &cobra.Command{
	Use:   "termai",
	Short: "AI-powered terminal assistant",
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Println("termAI iniciado 🚀")
		fmt.Println("Digite um comando (ou 'exit' para sair)")

		scanner := bufio.NewScanner(os.Stdin)

		for {
			fmt.Print("termai > ")

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

			if result.Error != "" {
				fmt.Println("Erro:", result.Error)
			}

			suggestion := analyzer.Analyze(result.Error)

			if suggestion != nil {
				fmt.Println("\n⚠️ ", suggestion.Title)
				fmt.Println(suggestion.Description)

				if len(suggestion.Actions) > 0 {
					fmt.Println("\nSugestões:")
					for _, action := range suggestion.Actions {
						fmt.Println(" -", action)
					}
				}
			}
		}
	},
}

func Execute() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Println(err)
		os.Exit(1)
	}
}
