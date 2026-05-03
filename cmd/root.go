package cmd

import (
	"fmt"
	"os"
	"bufio"

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
	
			fmt.Println("Você digitou:", input)
		}
	},
}

func Execute() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Println(err)
		os.Exit(1)
	}
}