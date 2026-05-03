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

	return nil
}
