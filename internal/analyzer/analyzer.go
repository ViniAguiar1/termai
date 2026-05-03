package analyzer

import "strings"

type Suggestion struct {
	Title       string
	Description string
	Actions     []string
}

func Analyze(errorOutput string) *Suggestion {
	err := strings.ToLower(errorOutput)

	// ENOSPC → disco cheio
	if strings.Contains(err, "no space left") || strings.Contains(err, "enospc") {
		return &Suggestion{
			Title:       "Disco cheio detectado",
			Description: "Seu sistema pode estar sem espaço disponível",
			Actions: []string{
				"df -h",
				"rm -rf ~/Library/Caches/*",
				"npm cache clean --force",
			},
		}
	}

	// comando não encontrado
	if strings.Contains(err, "command not found") {
		return &Suggestion{
			Title:       "Comando não encontrado",
			Description: "O comando digitado não existe no sistema",
			Actions: []string{
				"Verifique se o comando está correto",
				"Instale a ferramenta necessária",
			},
		}
	}

	if strings.Contains(err, "address already in use") || strings.Contains(err, "eaddrinuse") {
		return &Suggestion{
			Title:       "Porta já está em uso",
			Description: "Outro processo já está utilizando essa porta",
			Actions: []string{
				"lsof -i :PORTA",
				"kill -9 <PID>",
				"Alterar a porta da aplicação",
			},
		}
	}

	if strings.Contains(err, "module not found") {
		return &Suggestion{
			Title:       "Módulo não encontrado",
			Description: "Dependência pode não estar instalada",
			Actions: []string{
				"pnpm install",
				"npm install",
				"Verificar package.json",
			},
		}
	}

	if strings.Contains(err, "permission denied") {
		return &Suggestion{
			Title:       "Permissão negada",
			Description: "Você não tem permissão para executar esse comando",
			Actions: []string{
				"chmod +x <arquivo>",
				"Executar com sudo (se necessário)",
			},
		}
	}

	return nil
}
