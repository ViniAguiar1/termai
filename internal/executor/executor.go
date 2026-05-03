package executor

import (
	"bytes"
	"os/exec"
)

type Result struct {
	Output   string
	Error    string
	ExitCode int
}

func Run(command string) Result {
	cmd := exec.Command("sh", "-c", command)

	var out bytes.Buffer
	var stderr bytes.Buffer

	cmd.Stdout = &out
	cmd.Stderr = &stderr

	err := cmd.Run()

	result := Result{
		Output:   out.String(),
		Error:    stderr.String(),
		ExitCode: 0,
	}

	if err != nil {
		// pega exit code real
		if exitErr, ok := err.(*exec.ExitError); ok {
			result.ExitCode = exitErr.ExitCode()
		} else {
			result.ExitCode = 1
		}
	}

	return result
}
