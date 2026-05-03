package analyzer

import "strings"

type Action struct {
	Label   string
	Command string
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
				{Label: "Verificar espaço em disco", Command: "df -h"},
				{Label: "Limpar cache do sistema (Mac)", Command: "rm -rf ~/Library/Caches/*"},
				{Label: "Limpar cache do npm", Command: "npm cache clean --force"},
			},
		}
	}

	// comando não encontrado
	if strings.Contains(err, "command not found") {
		return &Suggestion{
			Title:       "Comando não encontrado",
			Description: "O comando digitado não existe no sistema",
			Actions: []Action{
				{Label: "Verificar se o comando está correto", Command: ""},
				{Label: "Instalar a ferramenta necessária", Command: ""},
			},
		}
	}

	// porta em uso
	if strings.Contains(err, "address already in use") || strings.Contains(err, "eaddrinuse") {
		return &Suggestion{
			Title:       "Porta já está em uso",
			Description: "Outro processo já está utilizando essa porta",
			Actions: []Action{
				{Label: "Listar processos na porta", Command: "lsof -i :PORTA"},
				{Label: "Finalizar processo (kill)", Command: "kill -9 <PID>"},
				{Label: "Alterar porta da aplicação", Command: ""},
			},
		}
	}

	// módulo não encontrado (Node)
	if strings.Contains(err, "module not found") {
		return &Suggestion{
			Title:       "Módulo não encontrado",
			Description: "Dependência pode não estar instalada",
			Actions: []Action{
				{Label: "Instalar dependências (pnpm)", Command: "pnpm install"},
				{Label: "Instalar dependências (npm)", Command: "npm install"},
				{Label: "Verificar package.json", Command: ""},
			},
		}
	}

	// permissão negada
	if strings.Contains(err, "permission denied") {
		return &Suggestion{
			Title:       "Permissão negada",
			Description: "Você não tem permissão para executar esse comando",
			Actions: []Action{
				{Label: "Dar permissão ao arquivo", Command: "chmod +x <arquivo>"},
				{Label: "Executar com sudo", Command: "sudo <comando>"},
			},
		}
	}

	return nil
}
