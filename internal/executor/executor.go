package executor

import (
	"bytes"
	"os/exec"
)

type Result struct {
	Output string
	Error  string
}

func Run(command string) Result {
	cmd := exec.Command("sh", "-c", command)

	var out bytes.Buffer
	var stderr bytes.Buffer

	cmd.Stdout = &out
	cmd.Stderr = &stderr

	err := cmd.Run()

	result := Result{
		Output: out.String(),
		Error:  stderr.String(),
	}

	if err != nil && result.Error == "" {
		result.Error = err.Error()
	}

	return result
}