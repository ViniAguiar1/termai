package cmd

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"
)

var rootCmd = &cobra.Command{
	Use:   "termai",
	Short: "AI-powered terminal assistant",
	Run: func(cmd *cobra.Command, args []string) {
		fmt.Println("termAI CLI iniciado 🚀")
	},
}

func Execute() {
	if err := rootCmd.Execute(); err != nil {
		fmt.Println(err)
		os.Exit(1)
	}
}