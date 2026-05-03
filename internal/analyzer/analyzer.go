package analyzer

import "strings"

type RiskLevel string

const (
	RiskLow    RiskLevel = "low"
	RiskMedium RiskLevel = "medium"
	RiskHigh   RiskLevel = "high"
)

type Action struct {
	Label                string
	Command              string
	Risk                 RiskLevel
	RequiresConfirmation bool
}

type Suggestion struct {
	Title       string
	Description string
	Actions     []Action
}

func Analyze(errorOutput string) *Suggestion {
	err := strings.ToLower(errorOutput)

	// ENOSPC → disco cheio
	if strings.Contains(err, "no space left") || strings.Contains(err, "enospc") {
		return &Suggestion{
			Title:       "Disco cheio detectado",
			Description: "Seu sistema pode estar sem espaço disponível",
			Actions: []Action{
				{Label: "Verificar espaço em disco", Command: "df -h", Risk: RiskLow},
				{
					Label:                "Limpar cache do sistema (Mac)",
					Command:              "rm -rf ~/Library/Caches/*",
					Risk:                 RiskHigh,
					RequiresConfirmation: true,
				},
				{
					Label:                "Limpar cache do npm",
					Command:              "npm cache clean --force",
					Risk:                 RiskMedium,
					RequiresConfirmation: true,
				},
			},
		}
	}

	// comando não encontrado
	if strings.Contains(err, "command not found") {
		return &Suggestion{
			Title:       "Comando não encontrado",
			Description: "O comando digitado não existe no sistema",
			Actions: []Action{
				{Label: "Verificar se o comando está correto", Command: "", Risk: RiskLow},
				{Label: "Instalar a ferramenta necessária", Command: "", Risk: RiskLow},
			},
		}
	}

	// porta em uso
	if strings.Contains(err, "address already in use") || strings.Contains(err, "eaddrinuse") {
		return &Suggestion{
			Title:       "Porta já está em uso",
			Description: "Outro processo já está utilizando essa porta",
			Actions: []Action{
				{Label: "Listar processos na porta", Command: "lsof -i :PORTA", Risk: RiskLow},
				{
					Label:                "Finalizar processo (kill)",
					Command:              "kill -9 <PID>",
					Risk:                 RiskHigh,
					RequiresConfirmation: true,
				},
				{Label: "Alterar porta da aplicação", Command: "", Risk: RiskLow},
			},
		}
	}

	// módulo não encontrado (Node)
	if strings.Contains(err, "module not found") || strings.Contains(err, "cannot find module") {
		return &Suggestion{
			Title:       "Módulo não encontrado",
			Description: "Dependência pode não estar instalada",
			Actions: []Action{
				{
					Label:                "Instalar dependências (pnpm)",
					Command:              "pnpm install",
					Risk:                 RiskMedium,
					RequiresConfirmation: true,
				},
				{
					Label:                "Instalar dependências (npm)",
					Command:              "npm install",
					Risk:                 RiskMedium,
					RequiresConfirmation: true,
				},
				{Label: "Verificar package.json", Command: "", Risk: RiskLow},
			},
		}
	}

	// permissão negada
	if strings.Contains(err, "permission denied") {
		return &Suggestion{
			Title:       "Permissão negada",
			Description: "Você não tem permissão para executar esse comando",
			Actions: []Action{
				{
					Label:                "Dar permissão ao arquivo",
					Command:              "chmod +x <arquivo>",
					Risk:                 RiskMedium,
					RequiresConfirmation: true,
				},
				{
					Label:                "Executar com sudo",
					Command:              "sudo <comando>",
					Risk:                 RiskHigh,
					RequiresConfirmation: true,
				},
			},
		}
	}

	return nil
}
