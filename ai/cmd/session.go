package cmd

import (
	"fmt"
	"io"
	"strconv"

	"github.com/ViniAguiar1/termai/ai/internal/analyzer"
	"github.com/ViniAguiar1/termai/ai/internal/executor"
)

type session struct {
	reader lineReader
}

func newSession(reader lineReader) *session {
	return &session{reader: reader}
}

func (s *session) run() error {
	fmt.Println("termAI iniciado 🚀")
	fmt.Println("Digite um comando (ou 'exit' para sair)")

	for {
		input, err := s.reader.ReadLine(buildPrompt())
		if err != nil {
			if err == io.EOF {
				fmt.Println()
				return nil
			}

			return err
		}

		if input == "" {
			continue
		}

		if input == "exit" {
			fmt.Println("Encerrando termAI...")
			return nil
		}

		s.handleCommand(input)
		fmt.Println("────────────────────────")
	}
}

func (s *session) handleCommand(input string) {
	result := executor.RunWithPTY(input)

	if result.Output != "" {
		fmt.Print(result.Output)
	}

	if result.ExitCode != 0 {
		fmt.Println(errorColor("❌ Erro:"), result.Error)
	} else if result.Error != "" {
		fmt.Println(result.Error)
	}

	errorOutput := result.Error
	if errorOutput == "" && result.ExitCode != 0 {
		errorOutput = result.Output
	}

	suggestion := analyzer.AnalyzeCommand(input, errorOutput)
	if suggestion == nil {
		return
	}

	fmt.Println()
	fmt.Println(warnColor("⚠️ " + suggestion.Title))
	fmt.Println(suggestion.Description)

	if len(suggestion.Actions) == 0 {
		return
	}

	fmt.Println()
	fmt.Println(infoColor("💡 Sugestões:"))

	for i, action := range suggestion.Actions {
		fmt.Printf("   [%d] %s%s\n", i+1, action.Label, riskSuffix(action))
	}

	fmt.Println()
	choice, err := s.reader.ReadLine("Escolha uma ação (Enter para ignorar): ")
	if err != nil || choice == "" {
		return
	}

	index, convErr := strconv.Atoi(choice)
	if convErr != nil || index <= 0 || index > len(suggestion.Actions) {
		fmt.Println("Opção inválida.")
		return
	}

	selected := suggestion.Actions[index-1]
	if selected.Command != "" {
		s.runAction(selected)
		return
	}

	printActionGuidance(selected)
}

func (s *session) runAction(action analyzer.Action) {
	if hasPlaceholder(action.Command) {
		fmt.Println(warnColor("Ação precisa de edição manual:"), action.Command)
		return
	}

	if action.RequiresConfirmation && !confirmAction(s.reader, action) {
		fmt.Println("Ação cancelada.")
		return
	}

	fmt.Println(infoColor("⚙️ Executando:"), action.Command)

	var execResult executor.Result
	if action.UpdatesSessionEnv {
		execResult = executor.RunAndUpdateSession(action.Command)
	} else {
		execResult = executor.Run(action.Command)
	}
	if execResult.Output != "" {
		fmt.Print(execResult.Output)
	}

	if execResult.ExitCode != 0 {
		fmt.Println(errorColor("❌ Erro:"), execResult.Error)
	}
}
