package executor

import (
	"bytes"
	"os"
	"os/exec"
	"strings"
	"sync"
)

type Result struct {
	Output   string
	Error    string
	ExitCode int
}

var (
	sessionEnvMu sync.RWMutex
	sessionEnv   = map[string]string{}
)

func Run(command string) Result {
	return runCommand(command)
}

func RunAndUpdateSession(command string) Result {
	const envMarker = "\n__TERMAI_ENV_START__\n"

	result := runCommand(command + `; __termai_exit=$?; printf '` + envMarker + `'; env -0; exit "$__termai_exit"`)
	if result.ExitCode != 0 {
		result.Output = stripEnvPayload(result.Output, envMarker)
		return result
	}

	output, envData, found := strings.Cut(result.Output, envMarker)
	result.Output = output

	if found {
		updateSessionEnv(envData)
	}

	return result
}

func runCommand(command string) Result {
	cmd := exec.Command("sh", "-c", command)
	cmd.Env = mergedEnv()

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

func mergedEnv() []string {
	sessionEnvMu.RLock()
	defer sessionEnvMu.RUnlock()

	envMap := make(map[string]string)
	for _, item := range os.Environ() {
		key, value, ok := strings.Cut(item, "=")
		if ok {
			envMap[key] = value
		}
	}

	for key, value := range sessionEnv {
		envMap[key] = value
	}

	merged := make([]string, 0, len(envMap))
	for key, value := range envMap {
		merged = append(merged, key+"="+value)
	}

	return merged
}

func updateSessionEnv(raw string) {
	items := strings.Split(raw, "\x00")

	sessionEnvMu.Lock()
	defer sessionEnvMu.Unlock()

	for _, item := range items {
		if item == "" {
			continue
		}

		key, value, ok := strings.Cut(item, "=")
		if !ok {
			continue
		}

		sessionEnv[key] = value
	}
}

func stripEnvPayload(output string, marker string) string {
	cleaned, _, found := strings.Cut(output, marker)
	if !found {
		return output
	}

	return cleaned
}
